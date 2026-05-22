# External ROM suites

The repository keeps synthetic ROM fixtures under version control, but official external ROM suites stay outside git in a repo-managed local store.

## Fetching ROMs

```bash
make fetch-test-roms
make fetch-test-roms FAMILIES=blargg
make fetch-test-roms FAMILIES="blargg acid"
```

- `make fetch-test-roms` fetches the pinned upstream source(s) from `crates/gb-test-runner/data/sources.toml` into temporary checkout(s), materializes the curated runnable store under `/test/`, and removes the raw checkout afterwards.
- By default it fetches `all`, but it can materialize one or more explicit families through `FAMILIES=...`.
- The pinned upstream source inventory and per-file SHA-256 hashes are recorded in `crates/gb-test-runner/data/sources.toml`; most rows come from `GBEmulatorShootout`, while source-specific exceptions such as DocBoy SameSuite, little-things-gb, gbmicrotest, docboy-dmg, docboy-cgb, docboy-cgb-dmg, and docboy-cgb-dmg-ext rows declare their materialized family/ROM alias explicitly.

## ROM store layout

`/test/` is organized by family:

```text
/test/acid/
/test/ax6/
/test/blargg/
/test/daid/
/test/docboy/
  dmg/
  cgb/
  cgb-dmg/
  cgb-dmg-ext/
/test/gbmicrotest/
/test/hacktix/
/test/mealybug-tearoom-tests/
/test/mooneye/
/test/samesuite/
```

Each curated family directory contains only the ROMs currently listed in the matching manifest under `crates/gb-test-runner/data/*.toml`.

Manifest cases default to `ExecutionMode::Strict` when `execution_mode` is omitted; declare `execution_mode` only for non-default `permissive` or `experimental` rows. Manifest cases can be skipped with `disabled = true` only when they also carry a non-empty `comment = "..."` explaining the rationale; use this for explicit overfit, duplicate, impossible, or CI-budget exceptions, not as a quiet way to remove a failing oracle.

## Running suites

```bash
make test-roms         # fetch if needed + run all local curated DMG/SGB suites
make test-roms-real-boot # fetch if needed + run all local curated DMG suites through verified RealBoot
make test-roms-extra   # fetch if needed + run the exploratory/internal extra DMG suites
make test-roms-extra-real-boot # fetch if needed + run the exploratory/internal extra DMG suites through verified RealBoot
make test-roms-docboy  # fetch if needed + run the large exploratory DocBoy single-machine suites
make test-roms-docboy-real-boot # fetch if needed + run the large exploratory DocBoy single-machine suites through verified RealBoot
make run-blargg        # curated Blargg DMG family (includes dmg_sound 01..12)
make run-blargg-cpu-instrs # Blargg CPU instruction chunk used by CI
make run-blargg-dmg-sound # Blargg DMG sound chunk used by CI
make run-blargg-timing-memory-oam # Blargg timing/memory/OAM chunk used by CI
make run-acid          # curated Acid DMG family
make run-ax6           # exploratory/internal AX6 DMG RTC suite
make run-samesuite     # exploratory/internal SameSuite DMG suite
make run-samesuite-cgb # exploratory/internal SameSuite CGB variant suite
make run-samesuite-sgb # informational SameSuite SGB command-transport suite
make run-magen-cgb     # exploratory/internal Magen CGB suite
make run-little-things-gb # exploratory/internal DocBoy little-things-gb DMG suite
make run-gbmicrotest   # exploratory/internal DocBoy gbmicrotest DMG suite
make run-docboy-dmg    # exploratory/internal DocBoy docboy/* DMG suite
make run-daid          # workflow-managed Daid DMG suite
make run-cpp           # curated cpp MBC3 subset
make run-hacktix       # curated hacktix DMG subset
make run-mealybug      # workflow-managed Mealybug-tearoom DMG suite
make run-mooneye       # workflow-managed Mooneye DMG acceptance suite
make run-mooneye-acceptance # Mooneye acceptance/manual chunk used by CI
make run-mooneye-mbc1-mbc5 # Mooneye emulator-only MBC1/MBC5 chunk used by CI
make run-mooneye-mbc2  # Mooneye emulator-only MBC2 chunk used by CI
make test-roms-cgb     # fetch if needed + run the promoted green local curated CGB suites
make test-roms-cgb-real-boot # fetch if needed + run the promoted green local curated CGB suites through verified RealBoot
make test-roms-cgb-extra # fetch if needed + run the exploratory/internal CGB suites
make test-roms-cgb-extra-real-boot # fetch if needed + run the exploratory/internal CGB suites through verified RealBoot
make run-cgb-smoke     # manifest-backed Phase 10 CGB smoke suite
make run-cgb-boot-div  # manifest-backed Phase 10 CGB boot DIV suite
make run-cgb-boot-hwio # exploratory/internal Phase 10 CGB boot HWIO suite
make run-mooneye-cgb # exploratory/internal Mooneye CGB PPU suite
make run-samesuite-cgb # exploratory/internal SameSuite CGB variant suite
make run-magen-cgb     # exploratory/internal Magen CGB suite
make run-mealybug-cgb  # exploratory/internal Mealybug CGB suite
make run-docboy-cgb     # exploratory/internal DocBoy native CGB suite
make run-docboy-cgb-dmg # exploratory/internal DocBoy CGB GB-compatible suite
make run-docboy-cgb-dmg-ext # exploratory/internal DocBoy CGB DMG-ext suite
make run-cgb-speed     # manifest-backed Phase 10 CGB KEY1/speed suite
make run-cgb-ppu-basic # manifest-backed Phase 10 CGB PPU baseline suite
make run-cgb-ppu-hard  # manifest-backed Phase 10 CGB PPU hardening suite
make run-cgb-dma       # manifest-backed Phase 10 CGB DMA suite
make run-cgb-audio-blargg # manifest-backed Phase 10 CGB Blargg audio suite
make run-cgb-audio-samesuite # manifest-backed Phase 10 CGB SameSuite audio suite
make run-cgb-rtc       # manifest-backed Phase 10 CGB MBC3 RTC suite
make phase9-determinism-smoke # replay/save-load smoke checks for Phase 2 and Phase 6 fixtures
make phase9-determinism-local # replay/save-load sample across CPU/interrupts, Mooneye Timer/DMA, Acid/Mealybug PPU, cartridge, and one APU Blargg case
make phase9-diff-cartridge    # compare Phase 6 cartridge artifacts against SameBoy case-bundle output
make phase9-diff-acid         # compare Acid framebuffer artifacts against LibSameBoy case-bundle output
make phase9-diff-mealybug     # compare the SameBoy-PASS Mealybug framebuffer subset against LibSameBoy case-bundle output
make phase9-diff-hacktix      # compare Hacktix framebuffer artifacts against LibSameBoy case-bundle output
make phase9-first-divergence-hacktix # capture Hacktix local/LibSameBoy first-divergence probe windows
```

The aggregate `make test-roms-real-boot`, `make test-roms-extra-real-boot`, `make test-roms-docboy-real-boot`, `make test-roms-cgb-real-boot`, and `make test-roms-cgb-extra-real-boot` ROM-suite targets are local-only validation lanes. They require `GB_CYCLE_BOOT_ROM_ROOT` to point at a private boot-ROM directory with canonical filenames derived from each case revision, such as `dmg_boot.bin`, `mgb_boot.bin`, `cgb_boot.bin`, or `cgbE_boot.bin`, set `GB_CYCLE_TEST_ROM_STARTUP=real-boot` while invoking the normal `run-*` suite targets, run clean `RealBoot` without direct-start `SkipBoot` or `CustomBoot` overlays, and start each case timeout after the `FF50` handoff. Manifests expose only `revision`, not a separate `boot_rom` selector: in default skip/custom-boot lanes no firmware bytes are read, and any revision-specific behavior must come from `revision` / `MachineConfig::revision`, which is also exposed by `gb-cli --revision` and `gb-desktop --revision`. Re-run `make test-roms` after a DMG RealBoot pass, `make test-roms-extra` after an extra DMG RealBoot pass, `make test-roms-docboy` after a DocBoy RealBoot pass, `make test-roms-cgb` after a promoted CGB RealBoot pass, or `make test-roms-cgb-extra` after an extra CGB RealBoot pass if you want the matching report to reflect the default manifest startup baseline again. The promoted `make test-roms` aggregate now also runs `samesuite-sgb` as informational SGB packet/multiplayer bring-up evidence and writes those rows to `/test/test-report.md`; SGB real-boot validation remains future work because SGB boot-ROM asset verification is not part of the current DMG/CGB RealBoot lane. The extra DMG aggregate target `make test-roms-extra-real-boot` currently drives `ax6-dmg-extra`, `samesuite-dmg-extra`, `little-things-gb-dmg-extra`, and `gbmicrotest-dmg-extra`, and writes `/test/test-report-extra.md`. The DocBoy aggregate target `make test-roms-docboy-real-boot` drives `docboy-dmg-extra`, `docboy-cgb-extra`, `docboy-cgb-dmg-extra`, and `docboy-cgb-dmg-ext-extra`, writes single-machine rows to `/test/test-report-docboy.md`, and leaves linked-session rows in `run_linked_session` stdout plus retained failure artifacts. The CGB aggregate target `make test-roms-cgb-real-boot` drives the same promoted-green CGB suite list as `make test-roms-cgb` and writes `/test/test-report.md`, while `make test-roms-cgb-extra` and `make test-roms-cgb-extra-real-boot` currently drive `cgb-boot-hwio`, `mooneye-cgb-extra`, `samesuite-cgb-extra`, `magen-cgb-extra`, `mealybug-tearoom-cgb-extra`, and `little-things-gb-cgb-extra`, keep running later child suites after earlier red rows, return non-zero if any child suite fails, and write `/test/test-report-extra.md`.

Each `make run-*` target is autosufficient and materializes its own curated family before execution. Heavy extra/internal or non-default-host targets (`run-ax6`, `run-samesuite`, `run-samesuite-cgb`, `run-samesuite-sgb`, `run-magen-cgb`, `run-little-things-gb`, `run-little-things-gb-cgb`, `run-gbmicrotest`, `run-docboy-dmg`, `run-docboy-cgb`, `run-cgb-boot-hwio`, `run-mooneye-cgb`, `run-mealybug-cgb`, `run-docboy-cgb-dmg`, and `run-docboy-cgb-dmg-ext`) invoke the runner through `cargo run --profile $(ROM_PROFILE)` with default `ROM_PROFILE=release-max`; override locally with `ROM_PROFILE=release make <target>` when iterating on compile time rather than final timing evidence.

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

# Run the DocBoy serial-link participant framebuffer sessions
cargo run -p gb-test-runner --bin run_linked_session -- --suite docboy-dmg-linked-extra

# Run the internal CGB-to-CGB IR smoke session
cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-cgb-ir-smoke

# Run the SameSuite CGB extra variant suite
cargo run -p gb-test-runner --bin run_rom_suite -- --suite samesuite-cgb-extra

# Run the SameSuite SGB informational packet/multiplayer suite
cargo run -p gb-test-runner --bin run_rom_suite -- --suite samesuite-sgb

# Run the Magen CGB extra suite
cargo run -p gb-test-runner --bin run_rom_suite -- --suite magen-cgb-extra

# Run the DocBoy native CGB extra suite
cargo run -p gb-test-runner --bin run_rom_suite -- --suite docboy-cgb-extra

# Run the DocBoy CGB GB-compatible extra suite
cargo run -p gb-test-runner --bin run_rom_suite -- --suite docboy-cgb-dmg-extra

# Run the DocBoy CGB DMG-ext extra suite
cargo run -p gb-test-runner --bin run_rom_suite -- --suite docboy-cgb-dmg-ext-extra

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

The runner updates `/test/test-report.md` with a `family | rom | status` table when a promoted curated family suite executes, using `✅`, `❌` and `ℹ️` in the status column, adding a `non-failing/total` summary in the header, and keeping each family's pinned source order from `crates/gb-test-runner/data/sources.toml`. `samesuite-sgb` is intentionally a promoted-report informational suite: its two `console = "sgb"` rows render after `samesuite/ppu/blocking_bgpi_increase.gb` according to the pinned source order, but they are not treated as Slice 1 pass/fail gates. Non-DocBoy extra/internal single-machine suites render the same table shape in `/test/test-report-extra.md`, currently `ax6-dmg-extra`, `samesuite-dmg-extra`, `samesuite-cgb-extra`, `magen-cgb-extra`, `mealybug-tearoom-cgb-extra`, `little-things-gb-cgb-extra`, `gbmicrotest-dmg-extra`, `little-things-gb-dmg-extra`, `cgb-boot-hwio`, and `mooneye-cgb-extra`, while the large DocBoy single-machine suites render to `/test/test-report-docboy.md`, currently `docboy-dmg-extra`, `docboy-cgb-extra`, `docboy-cgb-dmg-extra`, and `docboy-cgb-dmg-ext-extra`, so exploratory evidence stays visible without changing the promoted aggregate report; their persisted single-machine status files use the shorter logical names `docboy-dmg.toml`, `docboy-cgb.toml`, `docboy-cgb-dmg.toml`, and `docboy-cgb-dmg-ext.toml` under `/test/.status/`. Linked-session suites such as `docboy-dmg-linked-extra` and `linked-cgb-ir-smoke` currently print their participant status to stdout and retain failure artifacts but do not append markdown rows. The legacy full Mooneye manifest suite `mooneye-acceptance-dmg-curated` may persist local status when run directly, but it is intentionally omitted from markdown aggregation because `make run-mooneye` and the promoted report are owned by the split `mooneye-dmg-acceptance-manual`, `mooneye-dmg-emulator-mbc1-mbc5`, and `mooneye-dmg-emulator-mbc2` suites. Same-ROM model variants are ordered DMG before GBC before SGB before SGB2, and manifest order is only the fallback for cases without a pinned source path. Empty report categories are not materialized, so a DocBoy-only run does not create or keep empty `test-report.md` or `test-report-extra.md` files.

Persisted `/test/.status/*.toml` rows are interpreted through their owning suite before any shared upstream family fallback, so a ROM reused by a promoted CGB suite and an extra DMG suite keeps separate report labels and stale extra-only model rows are pruned from promoted suite status files when that suite is updated. Legacy rows that stored the upstream full path are normalized atomically from the matched manifest row, preserving both the manifest report family and the stripped ROM label together.

For upstream rows whose report label includes a model suffix, the associated manifest case must carry both `console = "dmg"` or `console = "cgb"` and `report_model_suffix = true`; this keeps rows such as `which.gb (DMG)` and `which.gb (GBC)` visible without adding a suffix to rows whose upstream label has none.

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

Workflow-managed DMG subset; mixes framebuffer fixtures, one multi-fixture framebuffer oracle for `ppu_scanline_bgp.gb`, an absolute grayscale fixture for `stop_instr.gb (DMG)`, and one informational framebuffer capture case `rom_and_ram.gb`. The `rom_and_ram.gb` row stays informational under `Permissive`: its `ROM+RAM` header declares legacy RAM-size code `0x01`, while gb-cycle intentionally runs it on the fixed `8 KiB` always-enabled No MBC RAM baseline; startup-dependent black-square rows are boot-logo tilemap residue, not an SRAM pass condition.

### Mealybug-tearoom

Workflow-managed DMG subset using committed framebuffer fixtures for the curated green cases from `GBEmulatorShootout`; exercised by the GitHub `test-roms` workflow.

The full local gate remains `mealybug-tearoom-dmg-curated` and keeps all 24 curated cases, including cases where gb-cycle passes but the current GBEmulatorShootout table marks SameBoy as `FAIL`.

The Phase `9` SameBoy differential uses the narrower built-in suite `mealybug-tearoom-dmg-sameboy-differential`, which excludes the nine Mealybug rows that GBEmulatorShootout updated on March 22, 2026 marks as SameBoy non-PASS: `mealybug-m3-lcdc-bg-en-change`, `mealybug-m3-lcdc-bg-map-change`, `mealybug-m3-lcdc-obj-size-change`, `mealybug-m3-lcdc-obj-size-change-scx`, `mealybug-m3-lcdc-tile-sel-change`, `mealybug-m3-lcdc-tile-sel-win-change`, `mealybug-m3-lcdc-win-en-change-multiple-wx`, `mealybug-m3-lcdc-win-map-change`, and `mealybug-m3-scy-change`.

Do not treat those excluded cases as gb-cycle regressions just because `mealybug-tearoom-dmg-curated` diverges from SameBoy; for Phase `9.3`, the full local fixture gate is accepted as the gb-cycle signal and the SameBoy divergence is recorded as an oracle limitation unless stronger hardware-facing evidence or another passing oracle supersedes it.

The extra/internal CGB companion suite `mealybug-tearoom-cgb-extra` runs the same 24 ROM programs on `console = "cgb"` with `report_model_suffix = true`, `timeout_frames = 30`, framebuffer fixtures under `crates/gb-test-runner/data/fixtures/mealybug-cgb/`, and `oracle = "framebuffer-rgb555-fixture"` for every CGB framebuffer row. The experimental `m3_lcdc_win_en_change_multiple_wx.gb` row uses the RGB555 oracle against the real-CGB-C/D `m3_lcdc_win_en_change_multiple_wx.png` capture, with path-specific quantization of sparse near-black, near-green, and forced-white capture noise before RGB555 rank comparison. The suite owns the byte-identical Mealybug coverage that used to appear under `docboy-cgb-dmg`, reports as `mealybug-tearoom-tests | ppu/*.gb (GBC)` in `/test/test-report-extra.md`, and mirrors the base DMG manifest's per-ROM timeout/startup shape instead of running a longer until-match CGB-only policy. The `m3_bgp_change_sprites.gb`, `m3_lcdc_bg_map_change.gb`, `m3_lcdc_obj_en_change.gb`, `m3_lcdc_obj_en_change_variant.gb`, `m3_lcdc_tile_sel_change.gb`, `m3_obp0_change.gb`, and `m3_scx_low_3_bits.gb` rows pin `startup = "custom-boot"` because their fixtures exercise the boot-logo tile bytes through BG, window, sprite/OBJ, or BG scroll paths while still avoiding a full private CGB boot ROM dependency.

### Mooneye

Workflow-managed DMG acceptance subset following the active `GBEmulatorShootout` `testroms/mooneye.py` acceptance list. Uses the upstream `mooneye` breakpoint/register result protocol instead of framebuffer oracles, with the documented manual sprite-priority exception handled by a committed framebuffer fixture; this is broad hardening evidence for the accepted Phase `9` closure matrix. The MBC1 `multicart_rom_8Mb.gb` row runs under the default strict execution mode because gb-cycle now identifies the tested MBC1M layout through the supported explicit subheader signature path rather than an experimental heuristic. The full built-in suite remains `mooneye-acceptance-dmg-curated`; CI runs the same case set through three filtered chunks, `mooneye-dmg-acceptance-manual`, `mooneye-dmg-emulator-mbc1-mbc5`, and `mooneye-dmg-emulator-mbc2`, so the mapper-heavy cases do not keep one Mooneye matrix job much longer than the smaller ROM-suite jobs.

## Extra DMG suites

```sh
make run-ax6
make run-samesuite
make run-little-things-gb
make run-gbmicrotest
make run-docboy-dmg
make test-roms-extra
make test-roms-docboy
make test-roms-extra-real-boot
make test-roms-docboy-real-boot
```

- `ax6-dmg-extra` is the extra/internal AX6 DMG MBC3 RTC suite, not a promoted DMG closure lane; its suite definition is `crates/gb-test-runner/data/ax6.toml`, it materializes upstream AX6 `rtc3test-1.gb`, `rtc3test-2.gb`, and `rtc3test-3.gb`, forces `console = "dmg"` with `report_model_suffix = true`, compares against committed DMG grayscale fixtures `crates/gb-test-runner/data/fixtures/ax6/rtc3test-*.dmg.png`, and writes rows such as `ax6 | rtc3test-1.gb (DMG) | ✅` to `/test/test-report-extra.md`.
- `run-ax6` materializes its upstream family with `make fetch-test-roms FAMILIES=ax6` before invoking `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite ax6-dmg-extra --failure-artifact-root .artifacts/ax6`; the upstream ROM inventory lives in `crates/gb-test-runner/data/sources.toml`, while the suite contract and local DMG fixture paths live in `crates/gb-test-runner/data/ax6.toml`.
- `samesuite-dmg-extra` is the extra/internal SameSuite DMG suite, not a promoted DMG closure lane; its suite definition is `crates/gb-test-runner/data/samesuite.toml`, it materializes the GBEmulatorShootout SameSuite `apu/div_write_trigger.gb` / `apu/div_write_trigger_10.gb` rows plus the DocBoy-backed `interrupt/ei_delay_halt.gb` row, forces `console = "dmg"`, keeps `report_model_suffix = true` only on the reused APU rows, compares against committed fixtures under `crates/gb-test-runner/data/fixtures/samesuite/`, and writes rows such as `samesuite | interrupt/ei_delay_halt.gb | ✅` to `/test/test-report-extra.md`.
- `run-samesuite` materializes the SameSuite family across all matching pinned sources with `make fetch-test-roms FAMILIES=samesuite` before invoking `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite samesuite-dmg-extra --failure-artifact-root .artifacts/samesuite`; the upstream ROM inventory lives in `crates/gb-test-runner/data/sources.toml`, while the suite contract and local DMG fixture paths live in `crates/gb-test-runner/data/samesuite.toml`.
- `samesuite-sgb` is the promoted-report informational SameSuite SGB suite for Phase 11 Slice 1; its suite definition is `crates/gb-test-runner/data/samesuite-sgb.toml`, it materializes `sgb/command_mlt_req.gb` and `sgb/command_mlt_req_1_incrementing.gb`, forces `console = "sgb"` so the runner uses the shared DMG core with `HostPlatform::Sgb`, captures framebuffers as `info-framebuffer`, and writes `ℹ️` rows to `/test/test-report.md` after `samesuite | ppu/blocking_bgpi_increase.gb | ...` by source order. These rows are observability only until Slice 5 implements `MLT_REQ`; use `make run-samesuite-sgb` to materialize `FAMILIES=samesuite` before invoking `run_rom_suite --suite samesuite-sgb`.
- `little-things-gb-dmg-extra` is the extra/internal DocBoy little-things-gb DMG suite, not a promoted DMG closure lane; its suite definition is `crates/gb-test-runner/data/little-things-gb.toml`, it materializes DocBoy `double-halt-cancel.gb` and `whichboot.gb`, forces `console = "dmg"`, keeps both report rows unsuffixed, compares against committed fixtures under `crates/gb-test-runner/data/fixtures/little-things-gb/`, marks `whichboot.gb` with `startup = "custom-boot"` so its boot-logo/map oracle sees the core DMG boot-logo VRAM seed, and writes rows such as `little-things-gb | whichboot.gb | ✅` to `/test/test-report-extra.md`.
- `run-little-things-gb` materializes the little-things-gb family from the pinned DocBoy source with `make fetch-test-roms FAMILIES=little-things-gb` before invoking `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite little-things-gb-dmg-extra --failure-artifact-root .artifacts/little-things-gb`; the upstream ROM inventory lives in `crates/gb-test-runner/data/sources.toml`, while the suite contract and local DMG fixture paths live in `crates/gb-test-runner/data/little-things-gb.toml`.
- `little-things-gb-cgb-extra` carries the DocBoy-sourced CGB `whichboot.gb` coverage split out from `docboy-cgb-extra`; its suite definition is `crates/gb-test-runner/data/little-things-gb-cgb.toml`, it materializes the same pinned DocBoy DMG `whichboot.gb` ROM into the logical `little-things-gb` family under `/test/little-things-gb/`, forces `console = "cgb"`, uses `startup = "custom-boot"`, `timeout_frames = 180`, `report_model_suffix = true`, and the committed fixture `crates/gb-test-runner/data/fixtures/little-things-gb-cgb/whichboot.png`, and writes `little-things-gb | whichboot.gb (GBC) | ✅` to `/test/test-report-extra.md`. The green path relies on the core CGB native old-licensee `$33` plus binary-zero new-licensee cartridge-entry timer/raster bucket, not a runner startup overlay, and has local canonical `cgb_boot.bin` `RealBoot` evidence through `make test-roms-cgb-extra-real-boot`; `make run-little-things-gb-cgb` materializes `FAMILIES=little-things-gb` before invoking `run_rom_suite --suite little-things-gb-cgb-extra`.
- `gbmicrotest-dmg-extra` is the extra/internal DocBoy `gbmicrotest` DMG suite, not a promoted DMG closure lane; its suite definition is `crates/gb-test-runner/data/gbmicrotest.toml`, its membership source of truth is DocBoy `tests/config/dmg.json` at `214905562590c35ba2bc41f36da3a5d636d99378` plus the six on-disk `gbmicrotest/dma` ROMs absent from that config (`dma_0x1000.gb`, `dma_0x9000.gb`, `dma_0xA000.gb`, `dma_0xC000.gb`, `dma_0xE000.gb`, and `dma_timing_a.gb`), it materializes `438` `gbmicrotest/...` ROMs from `tests/roms/dmg/gbmicrotest`, and checks the generic memory-byte oracle `$FF82 == $01`.
- `gbmicrotest/interrupts/is_if_set_during_ime0.gb` is the only `gbmicrotest-dmg-extra` row with a `2,000,000` T-cycle timeout because the source intentionally clears `IF`, leaves `IME=0`, then burns a full `BC` spin before reading `IF`; the remaining rows keep the normal `1,000,000` T-cycle budget.
- `gbmicrotest-dmg-extra` keeps ordinary rows startup-neutral so `make run-gbmicrotest` and `make test-roms-extra` use the default plain `SkipBoot` path, while `make test-roms-extra-real-boot` reruns the same manifest with `GB_CYCLE_TEST_ROM_STARTUP=real-boot`. The reset-facing `boot/poweron_*` rows plus the explicitly allowlisted PPU timing probes declare `startup = "custom-boot"` so they get the DMG boot-logo VRAM/map seed and reset-facing PPU CPU-bus publication instead of a runner-only PPU-profile workaround, while ordinary rows keep the same direct-start contract as gb-cli and gb-desktop and Mooneye `boot_hwio-dmgABCmgb`.
- `run-gbmicrotest` materializes its upstream family with `make fetch-test-roms FAMILIES=gbmicrotest` before invoking `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite gbmicrotest-dmg-extra --failure-artifact-root .artifacts/gbmicrotest`; the DocBoy source pin, explicit `family = "gbmicrotest"` / `rom = ...` aliases, and per-ROM hashes live in `crates/gb-test-runner/data/sources.toml`.
- `docboy-dmg-extra` is the extra/internal DocBoy `docboy/*` DMG suite, not a promoted DMG closure lane; its suite definition is `crates/gb-test-runner/data/docboy-dmg.toml`, its membership source of truth is DocBoy `tests/config/dmg.json` at `214905562590c35ba2bc41f36da3a5d636d99378`, it materializes the enabled unique single-machine `docboy/...` rows from `tests/roms/dmg/docboy`, excludes only explicitly disabled rows with manifest comments such as the CI-budget `docboy/mbc/huc3/huc3_tick.gb` row and the DocBoy-overfit `docboy/apu/ch1_period_sweep/change_period_nr14_during_recalc_delay*.gb` rows, excludes the duplicate `docboy/boot/boot_hram.gb` config row, checks memory rows with `$FFF0 == $01` plus fail-fast `$FFF0 == $02`, and checks visual rows with the DocBoy until-match framebuffer fixture oracle against committed image fixtures under `crates/gb-test-runner/data/fixtures/docboy/dmg/` while requiring configured exact `check_at_tcycles` probes to be reached before any final-frame fallback can pass.
- `docboy-dmg-extra` carries deterministic t-cycle joypad stimuli for input-driven rows and unique report labels for same-ROM variants that differ only by input or fixture; it intentionally omits explicit `startup` fields so `make run-docboy-dmg` and `make test-roms-docboy` use the default SkipBoot path, while `make test-roms-docboy-real-boot` reruns the same single-machine manifest with `GB_CYCLE_TEST_ROM_STARTUP=real-boot`.
- `docboy-dmg-linked-extra` covers the two DocBoy serial two-player `rom2` rows as four participant-scoped framebuffer checks, runs through `run_linked_session` with topology `dmg04`, resolves its ROMs from the same `docboy-dmg` materialized family under `/test/docboy/dmg/`, enforces participant exact `check_at_tcycles` probes before any final-frame fallback can pass, and honors `GB_CYCLE_TEST_ROM_STARTUP=skip-boot|custom-boot|real-boot` for local startup overrides; these linked participant results currently appear in the command output and retained artifacts rather than `/test/test-report-docboy.md`. The `serial_two_players_basic_transfer_slave_sc_00.gb` participant pair is intentionally kept as a blocking hardware-question row even though it is red in the current core, because DocBoy asserts a slave-side transfer can complete when the slave writes `SC = 0` while Pan Docs describes external-clock receivers as needing `SC.7 = 1`; do not retune serial gating for that row without stronger hardware-facing evidence.
- `linked-cgb-ir-smoke` is an internal linked-session CGB IR smoke suite backed by repo-owned synthetic fixtures in `crates/gb-test-runner/data/fixtures/linked/cgb-ir/`; it uses topology `cgb-ir`, two native-CGB participants, and a participant-scoped serial oracle (`receiver` emits `$B2`) to validate only CGB-to-CGB `RP` emitter/sensor routing after the deterministic read-enable warmup.
- `run-docboy-dmg` materializes the DocBoy family with `make fetch-test-roms FAMILIES=docboy-dmg` before invoking `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite docboy-dmg-extra --failure-artifact-root .artifacts/docboy-dmg` and `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_linked_session -- --suite docboy-dmg-linked-extra --failure-artifact-root .artifacts/docboy-dmg-linked`; it keeps running the linked session even when the single-machine DocBoy lane is already red, then exits non-zero if either command failed. The DocBoy source pin, explicit `family = "docboy-dmg"` / `rom = ...` aliases, fixture hashes, and linked-only ROM hashes live in `crates/gb-test-runner/data/sources.toml`.

## CGB suites

```sh
make run-cgb-smoke
make run-cgb-boot-div
make run-cgb-boot-hwio
make run-mooneye-cgb
make run-mealybug-cgb
make run-docboy-cgb
make run-docboy-cgb-dmg
make run-docboy-cgb-dmg-ext
make run-cgb-speed
make run-cgb-ppu-basic
make run-cgb-ppu-hard
make run-cgb-dma
make run-cgb-audio-blargg
make run-cgb-audio-samesuite
make run-cgb-rtc
cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-cgb-ir-smoke
make test-roms-cgb
make test-roms-cgb-real-boot
make test-roms-cgb-extra
make test-roms-cgb-extra-real-boot
```

- `cgb-smoke` is the Phase `10` CGB catalog suite, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-smoke.toml`, the cases use the default centralized CGB `SkipBoot` handoff like the DMG `which.gb` informational lane, and `make run-cgb-smoke` fetches `mooneye acid` before invoking `run_rom_suite` without requiring boot ROM assets. It is part of the promoted `make test-roms-cgb` aggregate and the GitHub `test-roms` matrix, while verified CGB `RealBoot` coverage for the same rows remains available through `make test-roms-cgb-real-boot`.
- `cgb-boot-div` is the Phase `10` CGB boot/DIV timing gate, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-boot-div.toml`, it uses the default centralized CGB `SkipBoot` handoff so CI does not need boot ROM assets, and `make run-cgb-boot-div` fetches `mooneye` before invoking `run_rom_suite`.
- `cgb-boot-div` currently runs Mooneye `misc/boot_div-cgbABCDE.gb` on `ConsoleModel::GameBoyColor` with a blocking `mooneye-result` oracle. It validates the CGB handoff/DIV timer baseline through the centralized CGB `SkipBoot` state; optional local `RealBoot` comparison remains available only through explicit `GB_CYCLE_TEST_ROM_STARTUP=real-boot` runs.
- `cgb-boot-hwio` is the Slice `6` extra/internal CGB HWIO suite, not a blocking DMG or promoted CGB aggregate signal; its suite definition is `crates/gb-test-runner/data/cgb-boot-hwio.toml`, its source row is `testroms/mooneye/misc/boot_hwio-C.gb` with pinned SHA-256 in `sources.toml`, it uses the default centralized CGB `SkipBoot` handoff so CI does not need boot ROM assets, `make run-cgb-boot-hwio` fetches `mooneye` before invoking `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite cgb-boot-hwio --failure-artifact-root .artifacts/cgb-boot-hwio`, and the current oracle is the Mooneye pass/fail signature after documenting the CGB compatibility-mode HWIO readbacks covered by that ROM. The suite is grouped under `make test-roms-cgb-extra` and `make test-roms-cgb-extra-real-boot`, and its persisted row renders in `/test/test-report-extra.md` rather than `/test/test-report.md`.
- `mooneye-cgb-extra` is the extra/internal Mooneye CGB PPU acceptance slice, not a promoted CGB aggregate signal; its suite definition is `crates/gb-test-runner/data/mooneye-cgb.toml`, it materializes 12 upstream `testroms/mooneye/acceptance/ppu/*.gb` manifest rows that mirror the DocBoy CGB-DMG Mooneye PPU subset plus `intr_2_mode0_timing.gb`, runs 10 active rows after disabling `lcdon_timing-GS.gb` and `vblank_stat_intr-GS.gb` because upstream documents them as expected CGB/AGB/AGS reds, keeps `intr_2_mode0_timing_sprites.gb` active because upstream expects it to pass on CGB and it now covers the CGB PPU/OBJ STAT-publication regression that previously failed locally with `TEST #63 FAILED`, forces `console = "cgb"`, uses `mooneye-result`, keeps the default centralized CGB `SkipBoot` handoff, and writes rows such as `mooneye | acceptance/ppu/intr_2_mode0_timing.gb | ✅` to `/test/test-report-extra.md`; `make run-mooneye-cgb` materializes `FAMILIES=mooneye` before invoking `run_rom_suite --suite mooneye-cgb-extra`.
- `samesuite-cgb-extra` is the extra/internal SameSuite CGB variant suite split out from the DocBoy native-CGB manifest, not a promoted CGB aggregate signal; its suite definition is `crates/gb-test-runner/data/samesuite-cgb.toml`, it materializes 10 DocBoy-sourced SameSuite CGB-only APU rows from `tests/roms/cgb/samesuite` into the logical `samesuite` family under `/test/samesuite/`, forces `console = "cgb"` with `report_model_suffix = true`, uses `timeout_frames = 180`, compares against committed RGB555 fixtures under `crates/gb-test-runner/data/fixtures/samesuite-cgb/`, selects `revision = "cpu-cgb-d"` for the `*-cgbDE` timing row and `revision = "cpu-cgb-e"` for `*-cgbE` rows, and writes rows such as `samesuite | apu/channel_1/channel_1_align_12-cgbE.gb (GBC) | ✅` to `/test/test-report-extra.md`. The skip/custom-boot reproduction command for a CGB-E row needs the core revision axis, for example `gb-cli run <rom> --model CGB --revision cpu-cgb-e`; RealBoot reproduction additionally needs `--boot-rom-dir <private-dir>` with `cgbE_boot.bin`, derived from the revision. It is grouped under `make test-roms-cgb-extra` and `make test-roms-cgb-extra-real-boot`; `make run-samesuite-cgb` materializes `FAMILIES=samesuite` before invoking `run_rom_suite --suite samesuite-cgb-extra`.
- `magen-cgb-extra` is the extra/internal Magen CGB suite split out from the DocBoy native-CGB manifest, not a promoted CGB aggregate signal; its suite definition is `crates/gb-test-runner/data/magen.toml`, it materializes eight DocBoy-sourced `tests/roms/cgb/magen` rows into the logical `magen` family under `/test/magen/`, forces `console = "cgb"`, uses fixed-time RGB555 fixture oracles with `timeout_tcycles = 5000000`, compares against committed fixtures under `crates/gb-test-runner/data/fixtures/magen/`, and writes rows such as `magen | bg_oam_priority.gbc | ✅` to `/test/test-report-extra.md`. It is grouped under `make test-roms-cgb-extra` and `make test-roms-cgb-extra-real-boot`; `make run-magen-cgb` materializes `FAMILIES=magen` before invoking `run_rom_suite --suite magen-cgb-extra`.
- `mealybug-tearoom-cgb-extra` is the extra/internal Mealybug CGB suite split out from the DocBoy CGB GB-compatible manifest, not a promoted CGB aggregate signal; its suite definition is `crates/gb-test-runner/data/mealybug-tearoom-tests-cgb.toml`, it materializes the same 24 GBEmulatorShootout Mealybug ROMs under `/test/mealybug-tearoom-tests/ppu/`, forces `console = "cgb"` with `report_model_suffix = true`, uses `timeout_frames = 30`, uses committed fixtures under `crates/gb-test-runner/data/fixtures/mealybug-cgb/`, and writes `mealybug-tearoom-tests | ppu/*.gb (GBC)` rows to `/test/test-report-extra.md`. The suite uses RGB555 framebuffer oracles for all CGB rows, including the experimental real-CGB-C/D fixture `m3_lcdc_win_en_change_multiple_wx.png` with path-specific quantization before RGB555 rank comparison; `make run-mealybug-cgb` materializes `FAMILIES=mealybug-tearoom-tests` before invoking `run_rom_suite --suite mealybug-tearoom-cgb-extra`.
- `docboy-cgb-extra` is the extra/internal DocBoy native CGB suite, not a promoted CGB closure lane; its suite definition is `crates/gb-test-runner/data/docboy-cgb.toml`, its membership source of truth is DocBoy `tests/config/cgb.json` at `214905562590c35ba2bc41f36da3a5d636d99378`, it materializes `6815` manifest rows from pinned `tests/roms/cgb` into the logical `docboy-cgb` family under `/test/docboy/cgb/`, skips the one upstream-disabled `daid/ppu_scanline_bgp.gbc` row that is absent from that pinned checkout, keeps the remaining `643` upstream-disabled rows visible but non-runnable, intentionally excludes duplicate Blargg `cgb_sound` rows already owned by `cgb-audio-blargg` plus Daid speed-switch / STOP rows already owned by `cgb-speed`, Acid `cgb-acid2.gbc` already owned by `cgb-ppu-basic`, Little Things `whichboot.gb` CGB custom-boot coverage split into `little-things-gb-cgb-extra`, and SameSuite rows either promoted into `cgb-audio-samesuite` / `cgb-ppu-basic` or split into `samesuite-cgb-extra` / `magen-cgb-extra`, forces `console = "cgb"` and relies on the default strict execution mode because these are ordinary native-CGB executions even though the suite itself is exploratory and report-isolated, checks `$FFF0 == $01` plus fail-fast `$FFF0 == $02` for memory rows, and uses committed DocBoy native-CGB PNG fixtures with RGB555 until-match framebuffer oracles for visual rows.
- `run-docboy-cgb` materializes its upstream family with `make fetch-test-roms FAMILIES=docboy-cgb` before invoking `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite docboy-cgb-extra --failure-artifact-root .artifacts/docboy-cgb`; it is included in `make test-roms-docboy` and the local RealBoot DocBoy aggregate, but intentionally stays outside `make test-roms-cgb`, `make test-roms-cgb-extra`, and GitHub `test-roms` until promoted intentionally.
- `docboy-cgb-dmg-extra` is the extra/internal DocBoy CGB GB-compatible suite, not a promoted CGB closure lane; its suite definition is `crates/gb-test-runner/data/docboy-cgb-dmg.toml`, its membership source of truth is DocBoy `tests/config/cgb_dmg_mode.json` at `214905562590c35ba2bc41f36da3a5d636d99378`, it materializes `467` manifest rows from `505` JSON entries in `tests/roms/cgb_dmg_mode` into the logical `docboy-cgb-dmg` family under `/test/docboy/cgb-dmg/` after skipping the one exact `boot/boot_vram.gb` manifest alias for the upstream `docboy/boot/boot_vram.gb` duplicate, omitting the DocBoy-packaged Mooneye `boot_div-cgbABCDE.gb` and `boot_regs-cgb.gb` rows because the promoted `cgb-boot-div` and `cgb-smoke` suites own those oracles, moving the 11 DocBoy-sourced Mooneye PPU duplicates into `mooneye-cgb-extra`, and moving the 24 byte-identical Mealybug rows into `mealybug-tearoom-cgb-extra`, forces `console = "cgb"`, keeps the default strict execution mode for rows that already enter the intended CGB-family GB-compatible mode, declares `execution_mode = "experimental"` only for the DocBoy KEY0 bit-2 rows `mode/mode_cgb_flag_84.gb`, `mode/mode_cgb_flag_85.gb`, `mode/mode_cgb_flag_86.gb`, and `mode/mode_cgb_flag_87.gb`, checks `$FFF0 == $01` plus fail-fast `$FFF0 == $02` for memory rows, and uses committed DocBoy CGB DMG-mode PNG fixtures for framebuffer until-match rows. The adjusted local evidence after removing the two promoted-overlap green rows was `69/501` passing before the 11 Mooneye PPU rows and 24 Mealybug rows moved out, so rerun this lane before quoting a current pass ratio; the target is expected to be a red extra/internal bring-up lane until compatibility-mode gaps are closed.
- `run-docboy-cgb-dmg` materializes its upstream family with `make fetch-test-roms FAMILIES=docboy-cgb-dmg` before invoking `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite docboy-cgb-dmg-extra --failure-artifact-root .artifacts/docboy-cgb-dmg`; it is included in `make test-roms-docboy` and the local RealBoot DocBoy aggregate, but intentionally stays outside `make test-roms-cgb`, `make test-roms-cgb-extra`, and GitHub `test-roms` until promoted intentionally.
- `docboy-cgb-dmg-ext-extra` is the extra/internal DocBoy CGB DMG-ext suite, not a promoted CGB closure lane; its suite definition is `crates/gb-test-runner/data/docboy-cgb-dmg-ext.toml`, its membership source of truth is DocBoy `tests/config/cgb_dmg_ext_mode.json` at `214905562590c35ba2bc41f36da3a5d636d99378`, it materializes exactly 26 upstream rows from `tests/roms/cgb_dmg_ext_mode/docboy` into the logical `docboy-cgb-dmg-ext` family under `/test/docboy/cgb-dmg-ext/` without retaining the redundant `docboy/` prefix, forces `console = "cgb"`, keeps the default strict execution mode for rows that already pass without the CGB DMG-ext policy, declares `execution_mode = "experimental"` only for `apu/pcm12_ch2_read.gb`, `hdma/gdma_basic_transfer.gb`, `hdma/hdma_basic_transfer.gb`, and `ppu/ocpd_write_read.gb`, checks `$FFF0 == $01` plus fail-fast `$FFF0 == $02`, and validates only the narrow experimental CGB DMG-ext register profile rather than full PGB/PSM behavior.
- `run-docboy-cgb-dmg-ext` materializes its upstream family with `make fetch-test-roms FAMILIES=docboy-cgb-dmg-ext` before invoking `cargo run --profile $(ROM_PROFILE) -q -p gb-test-runner --bin run_rom_suite -- --suite docboy-cgb-dmg-ext-extra --failure-artifact-root .artifacts/docboy-cgb-dmg-ext`; it is included in `make test-roms-docboy` and the local RealBoot DocBoy aggregate, but intentionally stays outside `make test-roms-cgb`, `make test-roms-cgb-extra`, and GitHub `test-roms` until promoted intentionally.
- `cgb-speed` is the Phase `10` Slice `2` CGB speed-domain suite, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-speed.toml`, and `make run-cgb-speed` fetches `daid blargg` before invoking `run_rom_suite`.
- `cgb-speed` now promotes Daid `stop_instr.gb (GBC)` to a blocking final `framebuffer-rgb555-grayscale-fixture` using `crates/gb-test-runner/data/fixtures/daid/stop_instr.gbc.png`, preserving the absolute solid-black STOP result through a grayscale decode of the CGB RGB555 framebuffer; `stop_instr_gbc_mode3.gb` is a blocking rank-normalized `framebuffer-rgb555-fixture` using `crates/gb-test-runner/data/fixtures/daid/stop_instr_gbc_mode3.png`, matching the SameBoy/GBEmulatorShootout PASS screen where CGB STOP entered during Mode `3` leaves the LCD displaying the PASS text; `speed_switch_timing_div.gbc`, `speed_switch_timing_ly.gbc`, and `speed_switch_timing_stat.gbc` are blocking rank-normalized `framebuffer-rgb555-fixture` oracles using their matching `crates/gb-test-runner/data/fixtures/daid/speed_switch_timing_*.png` artifacts. These Daid cases use a `180`-frame budget so the terminal STOP or timing output has been presented to the framebuffer before comparison. Blargg `interrupt_time.gb` is promoted to a blocking `blargg-console-contains` oracle with expected text `Passed` and a `1800`-frame budget, because the CGB run emits its result through the upstream BG-map console rather than serial. Every current `cgb-speed` row now has a blocking oracle.
- `cgb-ppu-basic` is the Phase `10` Slice `4` CGB PPU baseline promotion suite, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-ppu-basic.toml`, and `make run-cgb-ppu-basic` fetches `samesuite daid acid hacktix` before invoking `run_rom_suite`.
- `cgb-ppu-basic` currently contains four blocking rows in roadmap order: SameSuite `ppu/blocking_bgpi_increase.gb`, using the `framebuffer-rgb555-fixture` oracle at `crates/gb-test-runner/data/fixtures/samesuite/ppu/blocking_bgpi_increase.png`; Daid `ppu_scanline_bgp.gb (GBC)`, using the `framebuffer-rgb555-fixture` oracle against `crates/gb-test-runner/data/fixtures/daid/ppu_scanline_bgp.gbc.png`; Acid `cgb-acid2.gbc`, using the `framebuffer-rgb555-fixture` oracle against `crates/gb-test-runner/data/fixtures/acid/cgb-acid2-cgb.png`; and Hacktix `bully.gb (GBC)`, using the `framebuffer-rgb555-fixture` oracle against `crates/gb-test-runner/data/fixtures/hacktix/bully.cgb.png` and `startup = "custom-boot"` for the CGB custom-boot logo tile seed without the DMG logo tilemap overlay. BullyGB's unconfirmed initial-`DIV` check is satisfied by the core header-aware CGB direct-start timer bucket rather than by any manifest timer override.
- `cgb-ppu-hard` is the Phase `10` Slice `9` native-CGB PPU hardening suite, not a repo-gated DMG closure lane; its ROM inventory and PNG fixture hashes are declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-ppu-hard.toml`, and `make run-cgb-ppu-hard` fetches `acid` before invoking `run_rom_suite`.
- `cgb-ppu-hard` currently runs Acid `cgb-acid-hell.gbc` on `ConsoleModel::GameBoyColor` with a blocking `framebuffer-rgb555-fixture` oracle against `crates/gb-test-runner/data/fixtures/acid/cgb-acid-hell.png`; the manifest intentionally uses the ordinary fixed `180`-frame budget rather than a runner-only stop condition so the captured framebuffer matches the `gb-cli` / `gb-desktop` screenshot path. The target is part of `make test-roms-cgb`, the GitHub `test-roms` matrix, and the local boot-ROM-backed `make test-roms-cgb-real-boot` aggregate.
- `cgb-dma` is the Phase `10` Slice `5` CGB GDMA/HDMA suite, not a repo-gated DMG closure lane; its ROM inventory and PNG fixture hashes are declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-dma.toml`, and `make run-cgb-dma` fetches `samesuite` before invoking `run_rom_suite`.
- `cgb-dma` currently contains four blocking SameSuite framebuffer rows: `dma/gbc_dma_cont.gb`, `dma/gdma_addr_mask.gb`, `dma/hdma_lcd_off.gb`, and `dma/hdma_mode0.gb`, each running on `ConsoleModel::GameBoyColor` with a `framebuffer-rgb555-fixture` oracle under `crates/gb-test-runner/data/fixtures/samesuite/dma/`. The target is part of `make test-roms-cgb`, the GitHub `test-roms` matrix, and the local boot-ROM-backed `make test-roms-cgb-real-boot` aggregate.
- `cgb-audio-blargg` is the Phase `10` Slice `7` CGB audio baseline suite, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-audio-blargg.toml`, and `make run-cgb-audio-blargg` fetches `blargg` before invoking `run_rom_suite`.
- `cgb-audio-blargg` contains the twelve upstream Blargg `cgb_sound` individual ROMs `01-registers.gb` through `12-wave.gb`, runs each on `ConsoleModel::GameBoyColor`, uses `blargg-memory-text-contains` with expected text `Passed`, retains memory-text plus snapshot failure artifacts, and is the first CGB audio ROM gate before promoting deeper SameSuite APU rows.
- `cgb-audio-samesuite` is the Phase `10` Slice `7` advanced CGB APU suite; its ROM inventory and PNG fixture hashes are declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-audio-samesuite.toml`, and `make run-cgb-audio-samesuite` fetches `samesuite` before invoking `run_rom_suite` with retained RGB555 framebuffer and snapshot artifacts. The manifest tracks all `61` SameSuite APU rows in roadmap coarse-to-fine order, and the now-green target is part of both `make test-roms-cgb` and the local boot-ROM-backed `make test-roms-cgb-real-boot` aggregate.
- `cgb-rtc` is the Phase `10` Slice `8` CGB MBC3 RTC suite; its ROM inventory and PNG fixture hashes are declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-rtc.toml`, and `make run-cgb-rtc` fetches `ax6` before invoking `run_rom_suite` with retained RGB555 framebuffer and snapshot artifacts. The manifest tracks AX6 `rtc3test-1.gb`, `rtc3test-2.gb`, and `rtc3test-3.gb` on `ConsoleModel::GameBoyColor` with blocking `framebuffer-rgb555-fixture` oracles and frame budgets of `1140`, `900`, and `2400`; the target is part of `make test-roms-cgb`, the GitHub `test-roms` matrix, and the local boot-ROM-backed `make test-roms-cgb-real-boot` aggregate.
- `linked-cgb-ir-smoke` is the post-Slice 10 CGB-to-CGB IR smoke suite, not part of `make test-roms-cgb` or the GitHub `test-roms` matrix. It runs through `run_linked_session` with topology `cgb-ir`, stores no external assets, and should remain an internal/core-linked confidence check until commercial or dedicated hardware-facing IR oracles justify promotion. `PokemonPikachuColor` and `PokemonMysteryGift` are accessory protocol tests rather than linked-session ROM-suite members.
- The GitHub `test-roms` workflow runs each promoted CGB suite target as its own matrix child (`cgb-smoke`, `cgb-boot-div`, `cgb-speed`, `cgb-ppu-basic`, `cgb-ppu-hard`, `cgb-dma`, `cgb-audio-blargg`, `cgb-audio-samesuite`, and `cgb-rtc`) rather than one serialized `make test-roms-cgb` job, matching the existing DMG shard pattern. The GitHub `test-roms-extra` workflow uses the same matrix shape for green non-DocBoy extra/internal suites: `ax6`, `samesuite`, `little-things-gb`, `gbmicrotest`, `cgb-boot-hwio`, `mooneye-cgb`, `samesuite-cgb`, `magen-cgb`, `mealybug-cgb`, and `little-things-gb-cgb`.
- Keep exploratory CGB suites outside the promoted DMG/SGB `make test-roms`, promoted `make test-roms-cgb`, and GitHub `test-roms` workflows until promoted intentionally; CGB failures during bring-up should produce retained artifacts without changing the accepted DMG/SGB report signal, while `make test-roms-cgb` aggregates the green CGB suite targets promoted by Phase `10` slices, `make test-roms-cgb-real-boot` reruns that same aggregate through verified CGB RealBoot for local closure evidence, and `make test-roms-cgb-extra` / `make test-roms-cgb-extra-real-boot` keep non-DocBoy internal CGB evidence such as `cgb-boot-hwio`, `mooneye-cgb-extra`, `samesuite-cgb-extra`, `magen-cgb-extra`, `mealybug-tearoom-cgb-extra`, and `little-things-gb-cgb-extra` in the separate extra report. The green non-DocBoy extra suites are also checked by the separate GitHub `test-roms-extra` workflow; `make test-roms-docboy` / `make test-roms-docboy-real-boot` keep the large DocBoy CGB evidence in the separate DocBoy report and out of both GitHub ROM workflows.

## CI integration

- `make ci` stays as the fast local pre-push gate and does not fetch or run external ROM suites; it includes the Rust checks plus the coverage threshold gate through `cargo cov-check`.
- `make test-roms` fetches the curated ROM store if needed and runs all local curated DMG/SGB suites currently wired in `Makefile`: `acid`, the full Blargg lane via the three `run-blargg-*` chunks, `daid`, `hacktix`, `cpp`, `mealybug-tearoom-tests`, the full Mooneye lane via the three `run-mooneye-*` chunks, and informational `samesuite-sgb`.
- GitHub uses three core validation workflows: `ci` for Rust checks plus coverage, `test-roms` for the promoted workflow-managed ROM subset (`acid`, `blargg-cpu-instrs`, `blargg-dmg-sound`, `blargg-timing-memory-oam`, `daid`, `hacktix`, `cpp`, `mooneye-acceptance`, `mooneye-mbc1-mbc5`, `mooneye-mbc2`, `mealybug-tearoom-tests`, `cgb-smoke`, `cgb-boot-div`, `cgb-speed`, `cgb-ppu-basic`, `cgb-ppu-hard`, `cgb-dma`, `cgb-audio-blargg`, `cgb-audio-samesuite`, and `cgb-rtc`), and `test-roms-extra` for the green non-DocBoy extra/internal subset (`ax6`, `samesuite`, `little-things-gb`, `gbmicrotest`, `cgb-boot-hwio`, `mooneye-cgb`, `samesuite-cgb`, `magen-cgb`, `mealybug-cgb`, and `little-things-gb-cgb`).
- The GitHub `test-roms` and `test-roms-extra` workflows fan those suites out through matrices; every matrix child performs its own checkout, Rust toolchain setup, and Rust cache restore because GitHub-hosted runners are isolated per job.

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
- `GB_CYCLE_TEST_ROM_STARTUP` — ROM-suite startup override for local ignored external harness runs and the `run_rom_suite` / `run_linked_session` CLIs; omit it to preserve each suite manifest's declared startup mode, use `skip-boot` to force plain direct boot, use `custom-boot` to force the console-family custom boot-logo VRAM seed (DMG tiles plus tilemap, CGB tiles only with clear tilemap), and use `real-boot` only with `GB_CYCLE_BOOT_ROM_ROOT` for clean boot-ROM execution that clears direct-start startup-memory and startup timer state.
