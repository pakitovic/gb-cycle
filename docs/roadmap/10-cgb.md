# Phase 10 — CGB implementation roadmap

Implement CGB as an extension of the existing DMG core, not as a second emulator. Every CGB slice must close with the current DMG ROM suite still green at its `167/167` DMG baseline, subsystem-level tests for the behavior touched by the slice, a dedicated CGB `gb-test-runner` suite that starts as exploratory and becomes repo-gated only once green and documented, and SameBoy or GBEmulatorShootout comparison whenever a clear observable exists.

Reference hierarchy: hardware behavior must come first from [Pan Docs](https://gbdev.io/pandocs/), [The Cycle-Accurate Game Boy Docs](https://github.com/AntonioND/giibiiadvance/blob/master/docs/TCAGBD.pdf), [Game Boy: Complete Technical Reference](https://gekkio.fi/files/gb-docs/gbctr.pdf), and the local subsystem rules in `docs/hardware/CGB.md`; `docs/core/MODEL-AXES.md`, `docs/TESTING.md`, and `docs/REFERENCES.md` define project routing, validation, and source-use policy.

ROM catalog and maturity signal: use [GBEmulatorShootout](https://gbdev.io/GBEmulatorShootout/) and [gbdev/GBEmulatorShootout test ROM definitions](https://github.com/gbdev/GBEmulatorShootout/tree/main/testroms) to select, group, and track CGB ROM suites, but do not treat GBEmulatorShootout results as hardware-behavior authority.

## Slice 0 — CGB infrastructure and DMG guardrail

- Keep this CGB roadmap separate from the DMG closure signal.
- Add dedicated CGB suites through the same repo-managed ROM-suite protocol as the current DMG suites, but do not mix them into the default DMG `test-roms` workflow until each suite is green and intentionally promoted.
- Preserve the current DMG suite at its `167/167` baseline as a mandatory gate after every CGB slice; the persisted `.roms/test/test-report.md` may count additional CGB rows after `test-roms-cgb` runs, but those rows must not redefine the DMG gate.
- Define the initial CGB suite buckets as `cgb-smoke`, `cgb-boot-div`, `cgb-boot-hwio`, `cgb-speed`, `cgb-ppu-basic`, `cgb-dma`, `cgb-audio-blargg`, `cgb-audio-samesuite`, `cgb-rtc`, and `cgb-ppu-hard`.

### Initial CGB suite definitions

The `slice_number` column lists every roadmap slice that uses the suite as an active CGB gate or explicitly documented exploratory/internal check; `suite_order` is the recommended implementation and promotion order inside each suite; `family` uses the same external ROM family naming convention as the `docs/hardware/PPU.md` `## Tests` table; `ROM` omits the leading family directory because `family` carries that namespace.

This table is the planning inventory, not an executable suite definition. Every listed CGB suite must be implemented with the same manifest, source, materialization, Makefile, reporting, and documentation protocol used by existing DMG curated suites before it can be treated as an active slice gate, even while it is still exploratory.

`cgb-boot-hwio` starts as exploratory/internal because GBEmulatorShootout currently carries `misc/boot_hwio-C.gb` as a commented Mooneye case; promote it to a blocking repo gate only after the expected post-boot HWIO values, pass/fail channel, and retained artifacts are documented in the CGB runner manifest.

### CGB ROM-suite infrastructure contract

- No Phase 10 CGB suite may be defined only as Rust code, ad hoc path lists, or one-off materialization logic; `cgb-smoke`, `cgb-boot-div`, `cgb-boot-hwio`, `cgb-speed`, `cgb-ppu-basic`, `cgb-dma`, `cgb-audio-blargg`, `cgb-audio-samesuite`, `cgb-rtc`, and `cgb-ppu-hard` must each have a repo-owned manifest under `crates/gb-test-runner/data/` before the suite is used for slice closure.
- `crates/gb-test-runner/data/sources.toml` is the authoritative upstream inventory for every ROM, fixture, screenshot, blob, or oracle artifact downloaded for those CGB suites; each entry must include the upstream path and `sha256`, and no CGB gate may depend on an undeclared file copied opportunistically from a fetched repository.
- Each CGB suite manifest must follow the current DMG manifest contract used by files such as `crates/gb-test-runner/data/mooneye.toml` and `crates/gb-test-runner/data/blargg.toml`: `version`, `family` or per-case `family` for mixed-family suites, `suite_name`, `subsystem`, one `[[case]]` per ROM, explicit `id`, relative `rom`, timeout, oracle, expected value or fixture where applicable, `console = "cgb"` or equivalent model metadata, startup mode when it differs from the default, and failure-artifact policy when the default is not enough.
- CGB suite materialization must reuse the existing curated-store pipeline: source selection filters by family, required files are verified before copying, missing or hash-mismatched files fail early, `.roms/test/<family>/...` remains the runner-facing layout, and `.roms/test/.status/<suite>.toml` plus `.roms/test/test-report.md` remain the persisted status/report outputs.
- CGB report rows must use the upstream `family` from the roadmap inventory table and manifest case, not the suite name, so a mixed suite such as `cgb-smoke` reports `acid | which.gb (GBC) | ℹ️` and `mooneye | misc/boot_regs-cgb.gb | ✅` instead of inventing a `cgb-smoke` report family.
- CGB manifests must carry enough console-family metadata to distinguish rows where GBEmulatorShootout labels the row with a model suffix; use `console = "dmg"` or `console = "cgb"` for the runner model and `report_model_suffix = true` only when the upstream row label includes `(DMG)` or `(GBC)`, as with `acid/which.gb (DMG)` and `acid/which.gb (GBC)`, and must not add a suffix to rows such as `mooneye/misc/boot_regs-cgb.gb` where the upstream row has none.
- Report ordering must continue to follow GBEmulatorShootout rather than local suite grouping: sort by known family rank, then by the pinned `sources.toml` ROM order inside that family, then by model variant order DMG before GBC for same-ROM rows, then by manifest order and lexical fallback for unknowns; mixed-family CGB suites must merge into this ordering rather than rendering as one pseudo-family block.
- Each suite needs a Makefile target with the `run-cgb-<suite-suffix>` naming pattern, for example `run-cgb-smoke`; the target must fetch or verify the required upstream families first through `make fetch-test-roms FAMILIES="..."`, then invoke `cargo run -p gb-test-runner --bin run_rom_suite -- --suite <suite> --failure-artifact-root .artifacts/<suite>` so the command used for slice closure is stable and reviewable.
- The Makefile must maintain an aggregate `test-roms-cgb` target alongside the existing DMG `test-roms` target; each newly defined `run-cgb-*` suite target must be added to `test-roms-cgb` in the same slice that introduces it, so the aggregate grows incrementally from `run-cgb-smoke` through the later CGB suites without changing the DMG gate.
- If a CGB suite uses a new upstream family such as SameSuite or AX6 RTC tests, add that family to `sources.toml`, curated-family selection, materialization tests, and the Makefile fetch/run target in the same change that introduces the suite manifest.
- Adding or promoting a CGB suite must update the runner list/help coverage, manifest parser coverage when new metadata is required, source-filter/materialization tests, and docs in `docs/TESTING.md` or `docs/testing/ROM-SUITES.md` so CGB suites remain discoverable like the current DMG suites.
- Exploratory status changes only the gate semantics, not the repo-management protocol: exploratory CGB suites still require `sources.toml` inventory, a data manifest, Makefile execution, local status reporting, and artifact retention before their results can be cited in a slice.
- A slice cannot close on a CGB ROM gate until the corresponding Makefile target has been run and the resulting report has the expected PASS/INFO/FAIL meaning documented by the suite manifest; `make ci` alone is never sufficient for a CGB ROM-suite gate.

| slice_number | suite | suite_order | family | ROM | domain | complexity |
| --- | --- | ---: | --- | --- | --- | --- |
| 1, 3, 6 | `cgb-smoke` | 1 | mooneye | `misc/boot_regs-cgb.gb` | Boot / startup registers | MEDIUM |
| 1, 3, 6 | `cgb-smoke` | 2 | acid | `which.gb (GBC)` | Model detection / informational framebuffer | VERY LOW |
| 2 | `cgb-speed` | 1 | daid | `stop_instr.gb (GBC)` | STOP / CGB forced blank / blocking absolute grayscale decode from RGB555 framebuffer | MEDIUM |
| 2 | `cgb-speed` | 2 | daid | `speed_switch_timing_div.gbc` | KEY1 / DIV timing / blocking RGB555 framebuffer fixture | HIGH |
| 2 | `cgb-speed` | 3 | blargg | `interrupt_time.gb` | CPU / interrupt timing | HIGH |
| 2 | `cgb-speed` | 4 | daid | `stop_instr_gbc_mode3.gb` | STOP / CGB Mode 3 / blocking RGB555 framebuffer fixture | HIGH |
| 2 | `cgb-speed` | 5 | daid | `speed_switch_timing_ly.gbc` | KEY1 / LY timing / blocking RGB555 framebuffer fixture | VERY HIGH |
| 2 | `cgb-speed` | 6 | daid | `speed_switch_timing_stat.gbc` | KEY1 / STAT timing / blocking RGB555 framebuffer fixture | VERY HIGH |
| 2, 6 | `cgb-boot-div` | 1 | mooneye | `misc/boot_div-cgbABCDE.gb` | Boot / timer DIV / blocking Mooneye result | MEDIUM |
| 4 | `cgb-ppu-basic` | 1 | samesuite | `ppu/blocking_bgpi_increase.gb` | PPU / palette MMIO | HIGH |
| 4 | `cgb-ppu-basic` | 2 | daid | `ppu_scanline_bgp.gb (GBC)` | PPU / live BGP | MEDIUM |
| 4 | `cgb-ppu-basic` | 3 | acid | `cgb-acid2.gbc` | PPU / CGB raster | HIGH |
| 4 | `cgb-ppu-basic` | 4 | hacktix | `bully.gb (GBC)` | PPU / visible VRAM seed / CGB OAM-DMA policy | HIGH |
| 5 | `cgb-dma` | 1 | samesuite | `dma/gdma_addr_mask.gb` | DMA / GDMA address mask | MEDIUM |
| 5 | `cgb-dma` | 2 | samesuite | `dma/hdma_lcd_off.gb` | DMA / HDMA LCD off | HIGH |
| 5 | `cgb-dma` | 3 | samesuite | `dma/hdma_mode0.gb` | DMA / HDMA HBlank | HIGH |
| 5 | `cgb-dma` | 4 | samesuite | `dma/gbc_dma_cont.gb` | DMA / CGB continuation | VERY HIGH |
| 6 | `cgb-boot-hwio` | 1 | mooneye | `misc/boot_hwio-C.gb` | Boot / post-boot HWIO registers | HIGH |
| 7 | `cgb-audio-blargg` | 1 | blargg | `cgb_sound/01-registers.gb` | APU / registers | MEDIUM |
| 7 | `cgb-audio-blargg` | 2 | blargg | `cgb_sound/02-len_ctr.gb` | APU / length counter | MEDIUM |
| 7 | `cgb-audio-blargg` | 3 | blargg | `cgb_sound/03-trigger.gb` | APU / trigger | HIGH |
| 7 | `cgb-audio-blargg` | 4 | blargg | `cgb_sound/04-sweep.gb` | APU / sweep | HIGH |
| 7 | `cgb-audio-blargg` | 5 | blargg | `cgb_sound/05-sweep_details.gb` | APU / sweep details | HIGH |
| 7 | `cgb-audio-blargg` | 6 | blargg | `cgb_sound/06-overflow_on_trigger.gb` | APU / sweep overflow | HIGH |
| 7 | `cgb-audio-blargg` | 7 | blargg | `cgb_sound/07-len_sweep_period_sync.gb` | APU / length-sweep sync | HIGH |
| 7 | `cgb-audio-blargg` | 8 | blargg | `cgb_sound/08-len_ctr_during_power.gb` | APU / power length | MEDIUM |
| 7 | `cgb-audio-blargg` | 9 | blargg | `cgb_sound/09-wave_read_while_on.gb` | APU / CH3 wave read | HIGH |
| 7 | `cgb-audio-blargg` | 10 | blargg | `cgb_sound/10-wave_trigger_while_on.gb` | APU / CH3 wave trigger | HIGH |
| 7 | `cgb-audio-blargg` | 11 | blargg | `cgb_sound/11-regs_after_power.gb` | APU / power register state | MEDIUM |
| 7 | `cgb-audio-blargg` | 12 | blargg | `cgb_sound/12-wave.gb` | APU / CH3 wave | HIGH |
| 7 | `cgb-audio-samesuite` | 1 | samesuite | `apu/channel_1/channel_1_align.gb` | APU / CH1 alignment | HIGH |
| 7 | `cgb-audio-samesuite` | 2 | samesuite | `apu/channel_2/channel_2_align.gb` | APU / CH2 alignment | HIGH |
| 7 | `cgb-audio-samesuite` | 3 | samesuite | `apu/channel_1/channel_1_align_cpu.gb` | APU / CH1 CPU alignment | HIGH |
| 7 | `cgb-audio-samesuite` | 4 | samesuite | `apu/channel_2/channel_2_align_cpu.gb` | APU / CH2 CPU alignment | HIGH |
| 7 | `cgb-audio-samesuite` | 5 | samesuite | `apu/channel_1/channel_1_duty.gb` | APU / CH1 duty | HIGH |
| 7 | `cgb-audio-samesuite` | 6 | samesuite | `apu/channel_2/channel_2_duty.gb` | APU / CH2 duty | HIGH |
| 7 | `cgb-audio-samesuite` | 7 | samesuite | `apu/channel_1/channel_1_volume.gb` | APU / CH1 volume | HIGH |
| 7 | `cgb-audio-samesuite` | 8 | samesuite | `apu/channel_2/channel_2_volume.gb` | APU / CH2 volume | HIGH |
| 7 | `cgb-audio-samesuite` | 9 | samesuite | `apu/channel_3/channel_3_first_sample.gb` | APU / CH3 first sample | HIGH |
| 7 | `cgb-audio-samesuite` | 10 | samesuite | `apu/channel_4/channel_4_align.gb` | APU / CH4 alignment | HIGH |
| 7 | `cgb-audio-samesuite` | 11 | samesuite | `apu/channel_4/channel_4_lfsr.gb` | APU / CH4 LFSR | HIGH |
| 7 | `cgb-audio-samesuite` | 12 | samesuite | `apu/channel_4/channel_4_lfsr15.gb` | APU / CH4 LFSR15 | HIGH |
| 7 | `cgb-audio-samesuite` | 13 | samesuite | `apu/channel_4/channel_4_equivalent_frequencies.gb` | APU / CH4 equivalent frequencies | HIGH |
| 7 | `cgb-audio-samesuite` | 14 | samesuite | `apu/channel_1/channel_1_delay.gb` | APU / CH1 delay | HIGH |
| 7 | `cgb-audio-samesuite` | 15 | samesuite | `apu/channel_2/channel_2_delay.gb` | APU / CH2 delay | HIGH |
| 7 | `cgb-audio-samesuite` | 16 | samesuite | `apu/channel_3/channel_3_delay.gb` | APU / CH3 delay | HIGH |
| 7 | `cgb-audio-samesuite` | 17 | samesuite | `apu/channel_4/channel_4_delay.gb` | APU / CH4 delay | HIGH |
| 7 | `cgb-audio-samesuite` | 18 | samesuite | `apu/channel_1/channel_1_duty_delay.gb` | APU / CH1 duty delay | HIGH |
| 7 | `cgb-audio-samesuite` | 19 | samesuite | `apu/channel_2/channel_2_duty_delay.gb` | APU / CH2 duty delay | HIGH |
| 7 | `cgb-audio-samesuite` | 20 | samesuite | `apu/channel_1/channel_1_freq_change.gb` | APU / CH1 frequency change | HIGH |
| 7 | `cgb-audio-samesuite` | 21 | samesuite | `apu/channel_2/channel_2_freq_change.gb` | APU / CH2 frequency change | HIGH |
| 7 | `cgb-audio-samesuite` | 22 | samesuite | `apu/channel_3/channel_3_freq_change_delay.gb` | APU / CH3 frequency change delay | HIGH |
| 7 | `cgb-audio-samesuite` | 23 | samesuite | `apu/channel_4/channel_4_freq_change.gb` | APU / CH4 frequency change | HIGH |
| 7 | `cgb-audio-samesuite` | 24 | samesuite | `apu/channel_4/channel_4_frequency_alignment.gb` | APU / CH4 frequency alignment | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 25 | samesuite | `apu/channel_1/channel_1_restart.gb` | APU / CH1 restart | HIGH |
| 7 | `cgb-audio-samesuite` | 26 | samesuite | `apu/channel_2/channel_2_restart.gb` | APU / CH2 restart | HIGH |
| 7 | `cgb-audio-samesuite` | 27 | samesuite | `apu/channel_3/channel_3_restart_delay.gb` | APU / CH3 restart delay | HIGH |
| 7 | `cgb-audio-samesuite` | 28 | samesuite | `apu/channel_4/channel_4_lfsr_restart.gb` | APU / CH4 LFSR restart | HIGH |
| 7 | `cgb-audio-samesuite` | 29 | samesuite | `apu/channel_1/channel_1_stop_restart.gb` | APU / CH1 STOP restart | HIGH |
| 7 | `cgb-audio-samesuite` | 30 | samesuite | `apu/channel_2/channel_2_stop_restart.gb` | APU / CH2 STOP restart | HIGH |
| 7 | `cgb-audio-samesuite` | 31 | samesuite | `apu/channel_3/channel_3_stop_delay.gb` | APU / CH3 STOP delay | HIGH |
| 7 | `cgb-audio-samesuite` | 32 | samesuite | `apu/channel_1/channel_1_sweep.gb` | APU / CH1 sweep | HIGH |
| 7 | `cgb-audio-samesuite` | 33 | samesuite | `apu/channel_1/channel_1_sweep_restart.gb` | APU / CH1 sweep restart | HIGH |
| 7 | `cgb-audio-samesuite` | 34 | samesuite | `apu/channel_1/channel_1_sweep_restart_2.gb` | APU / CH1 sweep restart 2 | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 35 | samesuite | `apu/channel_1/channel_1_stop_div.gb` | APU / CH1 STOP DIV | HIGH |
| 7 | `cgb-audio-samesuite` | 36 | samesuite | `apu/channel_2/channel_2_stop_div.gb` | APU / CH2 STOP DIV | HIGH |
| 7 | `cgb-audio-samesuite` | 37 | samesuite | `apu/channel_3/channel_3_stop_div.gb` | APU / CH3 STOP DIV | HIGH |
| 7 | `cgb-audio-samesuite` | 38 | samesuite | `apu/channel_1/channel_1_volume_div.gb` | APU / CH1 volume DIV | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 39 | samesuite | `apu/channel_2/channel_2_volume_div.gb` | APU / CH2 volume DIV | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 40 | samesuite | `apu/channel_4/channel_4_volume_div.gb` | APU / CH4 volume DIV | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 41 | samesuite | `apu/div_write_trigger.gb` | APU / DIV write trigger | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 42 | samesuite | `apu/div_write_trigger_10.gb` | APU / DIV write trigger 10 | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 43 | samesuite | `apu/div_write_trigger_volume.gb` | APU / DIV write trigger volume | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 44 | samesuite | `apu/div_write_trigger_volume_10.gb` | APU / DIV write trigger volume 10 | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 45 | samesuite | `apu/div_trigger_volume_10.gb` | APU / DIV trigger volume 10 | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 46 | samesuite | `apu/channel_1/channel_1_nrx2_glitch.gb` | APU / CH1 NRx2 glitch | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 47 | samesuite | `apu/channel_2/channel_2_nrx2_glitch.gb` | APU / CH2 NRx2 glitch | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 48 | samesuite | `apu/channel_1/channel_1_restart_nrx2_glitch.gb` | APU / CH1 restart NRx2 glitch | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 49 | samesuite | `apu/channel_2/channel_2_restart_nrx2_glitch.gb` | APU / CH2 restart NRx2 glitch | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 50 | samesuite | `apu/channel_3/channel_3_shift_delay.gb` | APU / CH3 shift delay | HIGH |
| 7 | `cgb-audio-samesuite` | 51 | samesuite | `apu/channel_4/channel_4_lfsr_15_7.gb` | APU / CH4 LFSR 15-to-7 | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 52 | samesuite | `apu/channel_3/channel_3_shift_skip_delay.gb` | APU / CH3 shift skip delay | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 53 | samesuite | `apu/channel_4/channel_4_lfsr_7_15.gb` | APU / CH4 LFSR 7-to-15 | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 54 | samesuite | `apu/channel_3/channel_3_restart_during_delay.gb` | APU / CH3 restart during delay | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 55 | samesuite | `apu/channel_4/channel_4_lfsr_restart_fast.gb` | APU / CH4 LFSR fast restart | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 56 | samesuite | `apu/channel_3/channel_3_restart_stop_delay.gb` | APU / CH3 restart STOP delay | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 57 | samesuite | `apu/channel_3/channel_3_wave_ram_sync.gb` | APU / CH3 wave RAM sync | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 58 | samesuite | `apu/channel_3/channel_3_wave_ram_locked_write.gb` | APU / CH3 wave RAM locked write | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 59 | samesuite | `apu/channel_3/channel_3_and_glitch.gb` | APU / CH3 AND glitch | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 60 | samesuite | `apu/channel_1/channel_1_nrx2_speed_change.gb` | APU / CH1 NRx2 speed change | VERY HIGH |
| 7 | `cgb-audio-samesuite` | 61 | samesuite | `apu/channel_2/channel_2_nrx2_speed_change.gb` | APU / CH2 NRx2 speed change | VERY HIGH |
| 8 | `cgb-rtc` | 1 | ax6 | `rtc3test-1.gb` | Cartridge / MBC3 RTC | MEDIUM |
| 8 | `cgb-rtc` | 2 | ax6 | `rtc3test-2.gb` | Cartridge / MBC3 RTC | MEDIUM |
| 8 | `cgb-rtc` | 3 | ax6 | `rtc3test-3.gb` | Cartridge / MBC3 RTC | HIGH |
| 9 | `cgb-ppu-hard` | 1 | acid | `cgb-acid-hell.gbc` | PPU / hard mid-scanline CGB | VERY HIGH |

### CGB suite oracle channels

This table defines the default acceptance channel and retained artifacts expected when a CGB suite is promoted from exploratory to repo-gated. It must not block early bring-up: exploratory suites may run with a provisional channel, but a suite cannot become a blocking gate until its manifest names the exact channel, oracle policy, timeout, pass/fail rule, and retained artifacts.

| suite | default acceptance channel | retained artifacts before repo-gated promotion |
| --- | --- | --- |
| `cgb-smoke` | Register/signature or runner status for `boot_regs-cgb`; informational framebuffer for `which.gb` | Startup CPU/MMIO snapshot, final runner status, and framebuffer artifact for informational cases |
| `cgb-boot-div` | Register/signature or typed post-boot timer snapshot | Startup trace window, `DIV`/timer snapshot, boot mode, and final runner status |
| `cgb-boot-hwio` | Typed post-boot HWIO snapshot, optionally cross-checked against Mooneye `misc/boot_hwio-C.gb` | HWIO snapshot artifact, RealBoot versus SkipBoot comparison, startup/handoff trace, and manifest-recorded expected values |
| `cgb-speed` | Timing trace plus ROM-declared visible output when present | `KEY1`/`DIV`/timer/PPU-STAT trace, CPU resume snapshot, serial or framebuffer output if emitted, and final runner status |
| `cgb-ppu-basic` | Framebuffer/screenshot oracle | PNG or framebuffer hash, palette/VRAM/PPU snapshot, and short PPU/fetcher trace on failure |
| `cgb-dma` | DMA trace/snapshot plus ROM-declared visible output when present | DMA register snapshots, transfer trace, bus/CPU blocking trace, VRAM/OAM before-after snapshot, and framebuffer or serial output if emitted |
| `cgb-audio-blargg` | ROM text/serial/screen status as declared in the manifest | Serial/text capture or framebuffer capture, APU register snapshot, and channel/mixer trace on failure |
| `cgb-audio-samesuite` | PCM register stream or runner-defined SameSuite status | `PCM12`/`PCM34` stream, APU event trace, channel state snapshots, speed-domain snapshot, and final runner status |
| `cgb-rtc` | ROM text/serial status plus RTC state snapshot | Text/serial capture, MBC3 RTC register snapshot, latched/unlatched state, injected time source, and persistence artifact when relevant |
| `cgb-ppu-hard` | Framebuffer/screenshot differential oracle | PNG or framebuffer hash, SameBoy/differential artifact when available, PPU mode/fetcher trace, and mid-scanline MMIO trace on failure |

### CGB framebuffer oracle color policy

Repo-gated CGB framebuffer suites must declare the color-space channel used for pass/fail comparison before promotion. Manifest rows that compare the core CGB sideband against a PNG fixture use `framebuffer-rgb555-fixture`, which decodes the raw logical `RGB555` framebuffer through the runner's deterministic 5-bit-to-8-bit RGB profile before rank-normalized comparison. The preferred core oracle is a raw logical `RGB555` framebuffer hash or snapshot; PNG artifacts may be retained for human review only after conversion through one documented deterministic runner profile, and frontend display correction, CGB LCD pigment simulation, GBA correction, host monitor profiles, or post-processing must not become the implicit oracle for core PPU correctness.

When a suite intentionally validates a converted image instead of rank-normalized raw `RGB555`, its manifest must name the conversion profile and keep that profile stable with the oracle artifact. Monochrome CGB cases that need an absolute shade such as `STOP` forced black should use `framebuffer-rgb555-grayscale-fixture`, which decodes the CGB RGB555 framebuffer into absolute grayscale before comparison; ordinary visual CGB PPU promotion should use raw logical color once Slice 4 owns palettes. This keeps `cgb-acid2`, Hacktix Bully, and `cgb-acid-hell` from mixing hardware PPU semantics with frontend color-management choices.

### CGB MMIO ownership matrix

This matrix assigns every CGB-only or CGB-reinterpreted MMIO surface to one owning slice; later slices may validate boot handoff or cross-subsystem effects, but they must not create a second source of truth for the same register.

| address | register | owner slice | state owner | required coverage |
| --- | --- | --- | --- | --- |
| `FF04-FF07` | `DIV`, `TIMA`, `TMA`, `TAC` CGB double-speed semantics | 2 only; Slice 7 may add regression tests but must not change ownership | Timer / divider counter and edge detector | Unit/integration tests for CPU-visible double-speed divider cadence, timer input-bit selection, edge detection across speed switches, `DIV` write effects, `TIMA` overflow/reload/IRQ ordering, STOP switch freeze/reset behavior, and proof that later serial/APU work consumes this contract without redefining it |
| `FF40` | `LCDC` CGB-reinterpreted bit `0` priority behavior | 4 | PPU layer-priority composer | Synthetic PPU tests for CGB native BG/window master priority, Non-CGB BG/window enable semantics, CGB compatibility policy that keeps CGB-family color/prioritization state separate from DMG silicon, and final BG/OBJ composition after object drawing priority has already selected the winning OBJ pixel |
| `FF46` | `DMA` / OAM DMA with CGB double-speed behavior | 5 functional DMA behavior, consuming the Slice 2 speed-domain contract | DMA controller / OAM DMA state | Unit/integration tests for OAM DMA duration and CPU blocking in normal speed versus double speed, first-byte commit timing, completion timing, HRAM accessibility, CGB bus arbitration by source bus, and proof that LCD timing, HDMA block duration, and APU frame sequencing do not inherit the OAM-DMA speed change |
| `FF47-FF49` | `BGP`, `OBP0`, `OBP1` in CGB compatibility mode | 4 runtime palette routing, boot-selected compatibility palette seed algorithm and override policy validated again in 6 | PPU compatibility palette adapter over CGB palette RAM | Synthetic tests for CGB compatibility rendering through CGB palette RAM, `BGP` indexing BG palette `0`, `OBP0`/`OBP1` indexing OBJ palettes `0`/`1`, CGB native color rendering not depending on DMG palette registers, Non-CGB still using the DMG grayscale path, deterministic compatibility palette seed selection, and no collapse from CGB compatibility mode to DMG silicon behavior |
| `FF4C` | `KEY0` / `SYS` | 3 descriptor, direct-boot policy, and lockable state shape; 6 real boot write, lock, and handoff closure | System model / boot handoff state | Slice 3 tests cover CGB versus Non-CGB availability and direct-boot header-derived mode without treating `KEY0` as an ordinary post-handoff runtime register; Slice 6 tests cover boot-ROM writes before `FF50`, lock-on-handoff, post-lock immutability, and RealBoot versus SkipBoot equivalence |
| `FF4D` | `KEY1` / `SPD` | 2 | Speed state / scheduler timing contract | Unit and ROM tests for prepare bit, current-speed readback, STOP transition, DIV/timer edge semantics, and PPU-facing timing consumers |
| `FF4F` | `VBK` | 3, consumed by PPU in 4 and HDMA in 5 | VRAM bank state routed through bus and PPU | Unit/integration tests for readback high bits, banked VRAM CPU access, PPU fetch bank use, and HDMA destination bank policy |
| `FF50` | `BANK` / boot ROM mapping control with CGB split boot windows and handoff lock effects | 6 | Boot controller / bus overlay router | Unit/integration tests for CGB boot overlay windows at `0000-00FF` and `0200-08FF`, cartridge visibility at `0100-01FF`, one-way unmap semantics, `FF50` handoff ordering at `0x0100`, `KEY0` lock-on-handoff, RealBoot versus SkipBoot equivalence, and DMG-family `FF50` behavior remaining unchanged |
| `FF01-FF02` | `SB`, `SC` CGB serial extension | 7, consuming Slice 2 speed-domain state | Serial controller | Unit/integration tests for `SC.1` high-speed bit, internal-clock rates `8192`, `16384`, `262144`, and `524288` Hz across normal/double speed, transfer start/progress/completion, `SC.7` clear timing, serial IRQ timing, external-clock completion, disconnected input policy, and proof that DMG serial semantics remain unchanged when CGB mode is disabled |
| `FF51-FF55` | `HDMA1`, `HDMA2`, `HDMA3`, `HDMA4`, `HDMA5` | 5 | CGB DMA controller | Unit/integration tests for `HDMA1-4` source/destination masking, `HDMA5` start/mode/length write semantics, readback active/inactive bits, cancel behavior, GDMA burst, HDMA HBlank advance, VBlank non-advance, LCD-off behavior, HDMA pause/resume when CPU `HALT` pauses execution, destination overflow, normal-speed versus double-speed block timing, bus/CPU impact, and an explicit non-gating policy for source-bank or `VBK` changes during active HDMA |
| `FF56` | `RP` | 7 register stub/readback only; full IR behavior deferred post-Slice 10 | CGB external port / infrared register state | Implement only the documented CGB-only MMIO baseline needed by boot HWIO and ordinary software probing: read/write latch for writable bits, documented readback for disabled receiver/no signal, emitting-bit state, and Non-CGB fallback; host-side light injection, analog sensor adaptation, peer IR transport, timing of real light pulses, and title-specific IR workflows are explicitly outside Phase 10 |
| `FF68-FF6B` | `BCPS`/`BGPI`, `BCPD`/`BGPD`, `OCPS`/`OBPI`, `OCPD`/`OBPD` | 4 | PPU CGB palette RAM and palette index registers | Unit/integration tests for index readback, auto-increment, BG/OBJ palette separation, RGB555 storage, Mode 3 data blocking, and failed-write auto-increment behavior |
| `FF6C` | `OPRI` | 4 object-priority algorithm using boot-latched mode; Slice 6 validates boot default and readback; post-boot visual write effects are out of ordinary Phase 10 unless backed by hardware evidence | PPU object-priority mode state plus `OPRI` MMIO latch/readback, separate from object selection and BG-over-OBJ composition | Synthetic PPU tests for Non-CGB X-coordinate object priority, CGB OAM-order object priority, boot-latched DMG-style priority on CGB-family silicon, CGB compatibility boot default, post-boot `OPRI` read/write latch without accepted visual priority mutation, and no accidental DMG-silicon conflation |
| `FF70` | `SVBK` / `WBK` | 3 | WRAM bank state routed through bus | Unit/integration tests for bank `0` mapping to bank `1`, high-bit readback, banked WRAM isolation, Echo RAM behavior, and DMG fallback |
| `FF72-FF75` | undocumented CGB registers | 3 | CGB misc IO state | Unit tests for distinct register identity, `FF72-FF74` read/write behavior, `FF75` writable bits `4-6`, initial/direct-boot value policy, and Non-CGB fallback |
| `FF76-FF77` | `PCM12`, `PCM34` | 7 | APU digital output taps | Unit/integration tests for read-only CGB exposure, per-channel digital nibble mapping, power/off behavior, double-speed independence of APU timing, and Non-CGB fallback |

## Slice 1 — CGB model, machine mode, and direct boot baseline

- Consolidate `ConsoleModel::GameBoyColor`, `OperatingMode::Cgb`, `OperatingMode::GbCompatible`, and `CapabilitySet` as the only model/mode decision surface.
- Choose CGB native versus CGB compatibility mode from the cartridge CGB header without conflating CGB compatibility mode with DMG silicon.
- Validate the CGB initial state with `SkipBoot` before depending on a real CGB boot ROM.
- Add a synthetic header/mode policy matrix before broader CGB bring-up so the project has one source of truth for `0x0143`, `ConsoleModel`, `OperatingMode`, `KEY0`, and loader policy.
- CGB gate: `cgb-smoke` through `make run-cgb-smoke`.
- Regression gate: DMG `167/167` plus model-axis and capability coherence tests.

### CGB cartridge header and mode policy matrix

This matrix belongs to Slice 1 for direct boot and is revalidated by Slice 6 under RealBoot. It is not a replacement for Pan Docs or boot-ROM evidence; it is the project policy for how the core maps cartridge metadata into model and mode state without creating a second CGB core.

| cartridge/header scenario | `ConsoleModel::GameBoyColor` direct-boot policy | `ConsoleModel::GameBoy` policy | required validation |
| --- | --- | --- | --- |
| `0x0143` bit `7 = 0` | Start as `OperatingMode::GbCompatible`, synthesize `KEY0` DMG-compatibility state, seed direct-boot compatibility palettes through the Slice 4 PPU adapter, and keep CGB-family silicon quirks rather than pretending the model is DMG | Stay Non-CGB/DMG; do not expose CGB MMIO or CGB-only capabilities | Synthetic direct-boot tests for mode, capabilities, CGB-only MMIO fallback, and no accidental DMG-silicon OAM corruption when the model is CGB |
| `0x0143 = 0x80` | Start as CGB native mode with CGB features unlocked and compatibility metadata preserved for loader/reporting | Stay DMG silicon; loader policy may warn or classify support, but hardware mode must not mutate to CGB | Synthetic direct-boot tests for CGB native mode, `KEY0` seed, `CapabilitySet`, and model-axis separation |
| `0x0143 = 0xC0` | Treat as CGB native mode for ordinary Phase 10 behavior while preserving the CGB-only header classification in metadata | Stay DMG silicon; strict loader policy may reject or report unsupported runtime, but the core must not fake CGB hardware | Synthetic direct-boot tests for CGB-only metadata, native CGB capabilities, and DMG rejection/reporting policy |
| `0x0143` bit `7 = 1` with PGB-like or otherwise unusual low bits | Do not silently enable PGB/PSM behavior in Phase 10; classify as unsupported or experimental until a dedicated post-Slice 10 hardware-research target defines behavior | Stay DMG silicon and keep CGB-only behavior unavailable | Unit tests that ordinary `$80`/`$C0` CGB behavior remains supported while PGB-like values do not mutate undocumented hardware state implicitly |

## Slice 2 — Double speed, `KEY1`, `STOP`, and CGB timing foundation

- Implement `KEY1` as explicit hardware state: prepare bit, current speed, and the `STOP`-driven speed switch transition.
- Treat this slice as the single owner for `KEY1`, double-speed state, `DIV`, CGB timer edge semantics, and PPU-facing timing contracts; leaving any of those ownership boundaries for Slice 3 or Slice 7 would force backtracking.
- Own the CGB timer contract here completely: divider cadence, selected timer input bit, falling-edge detection, `DIV` write effects, `TIMA` overflow/reload/IRQ ordering, and STOP/speed-switch interactions are Slice 2 behavior, not Slice 7 behavior.
- Keep the LCD/PPU temporal domain correct; do not model double speed as a superficial frame multiplier.
- Keep `speed_switch_timing_ly` and `speed_switch_timing_stat` in this slice: they validate the shared timing contract, not the full CGB PPU feature set, and moving them to Slice 4 would hide a CPU-only `KEY1` implementation until too late.
- Make CPU, timer/DIV, PPU/LY/STAT, DMA, serial, and APU consume a shared speed-domain contract where their hardware behavior depends on CGB speed state, even when later slices still provide placeholder or partial CGB behavior for the subsystem-specific details.
- Add a minimal PPU/STAT timing bridge in this slice so LY and STAT-observable speed-switch timing is scheduled by the same contract that Slice 4 will later consume for full CGB PPU rendering.
- Add internal tests for the `KEY1` / `STOP` transition before relying on Daid ROMs: armed versus unarmed `STOP`, prepare-bit clear semantics, current-speed readback, exact transition duration policy, CPU resume point, and regression coverage that DMG `STOP` wake behavior is not rewritten by the CGB speed path.
- Add internal CGB `STOP` joypad-wake tests for the non-speed-switch path: selected `P1` rows must allow P10-P13 lines to wake STOP, disabled rows must not invent a wake source, IF/IE/IME handling must remain CPU/interrupt-owned, and the DMG STOP wake regression must stay green.
- Add internal `DIV` / timer tests for double-speed switching: `DIV` reset or freeze during the switch window, timer edge detection across the speed transition, `TIMA` overflow/reload/IRQ ordering when the selected timer bit changes, and parity between natural divider ticks and switch-induced divider effects.
- Add internal PPU/lock observability tests for `STOP`: start `STOP` during Mode `0`, Mode `1`, Mode `2`, and Mode `3`, assert the documented visible memory-lock / black-pixel behavior exposed through the minimal PPU bridge, and keep these tests as the contract that Slice 4 must preserve when the full CGB PPU renderer lands.
- Add scheduler-domain tests proving that the speed state changes CPU-visible speed-domain behavior and serial/OAM-DMA cadence while LCD timing, HDMA block duration, CPU-visible `DIV` read cadence, and APU frame sequencing remain on their documented CGB domains instead of being multiplied by a generic speed factor.
- Add a CGB-mode CPU and interrupt smoke suite before treating timing bring-up as stable: run focused instruction/flag, `IME` / `EI` / `DI` / `RETI`, interrupt priority, `HALT`, timer IRQ, and `STOP` cases under `ConsoleModel::GameBoyColor` and CGB operating modes to prove CGB mode reuses the proven SM83 core semantics instead of accidentally forking CPU behavior.
- Status note while Slice `2` is under implementation: the `cgb-speed` manifest and `make run-cgb-speed` target now have blocking oracles for every current row. Daid `stop_instr.gb (GBC)` is promoted to a blocking `framebuffer-rgb555-grayscale-fixture` for final solid-black STOP output through the CGB RGB555 channel, `stop_instr_gbc_mode3.gb` is promoted to a blocking `framebuffer-rgb555-fixture` for the SameBoy/GBEmulatorShootout PASS screen that remains visible when CGB STOP enters during Mode `3`, `speed_switch_timing_div.gbc`, `speed_switch_timing_ly.gbc`, and `speed_switch_timing_stat.gbc` are promoted to blocking rank-normalized RGB555 framebuffer fixtures, and Blargg `interrupt_time.gb` is promoted to a blocking BG-map console-text `Passed` oracle. The `cgb-boot-div` manifest and `make run-cgb-boot-div` target are now wired as the Slice `2` direct-boot DIV/timer gate with Mooneye `misc/boot_div-cgbABCDE.gb` and a blocking `mooneye-result` oracle; Slice `6` still owns full CGB `RealBoot` equivalence for the same domain.
- CGB gate order: first `cgb-speed` through `make run-cgb-speed`, then `cgb-boot-div` through `make run-cgb-boot-div`; both targets must pass before claiming strict Slice `2` closure.
- Regression gate: DMG `167/167` plus focused DMG `STOP` tests so the CGB path cannot rewrite DMG semantics.

## Slice 3 — VRAM/WRAM banking and CGB registers

- Implement two VRAM banks selected by `VBK`.
- Implement banked WRAM selected by `SVBK`, including the hardware behavior where selecting bank `0` maps bank `1`.
- Implement the Slice 3-owned CGB MMIO surface from the ownership matrix: `KEY0` descriptor behavior, direct-boot policy, and lockable state shape, plus `VBK`, `SVBK`, `FF72`, `FF73`, `FF74`, and `FF75`; treat `KEY1` as already owned by Slice 2, leave `OPRI` behavior to Slice 4, leave `RP` and `PCM12`/`PCM34` to Slice 7, and do not reimplement speed semantics here.
- Do not model `KEY0` as an ordinary runtime CGB register in Slice 3: direct boot may derive the final operating mode from the cartridge header, but real boot writes, `FF50`-triggered lock, and post-lock immutability are Slice 6 responsibilities.
- Keep DMG fallback explicit: CGB-only MMIO reads return `0xFF` or the documented non-CGB value and writes do not mutate nonexistent state.
- CGB gate: `cgb-smoke` through `make run-cgb-smoke`.
- Add synthetic unit and integration tests for banking because GBEmulatorShootout does not cover enough of this layer in isolation.
- Regression gate: DMG `167/167` plus DMG-only/CGB-only MMIO descriptor and read/write tests.
- Status: Slice `3` is implemented and validated. The core has native-CGB CPU-visible VRAM bank switching through `VBK`, WRAM bank switching through `SVBK` with raw bank `0` mapping to effective bank `1`, bus-owned `FF72-FF75` read/write behavior, and locked direct-boot `KEY0` state synthesized from the loaded cartridge header; closure validation covered focused synthetic tests, `make run-cgb-smoke`, the DMG `make test-roms` no-regression gate, `make test-roms-cgb` with report `176/176`, and `make ci`.

## Slice 4 — CGB PPU baseline, palettes, and tile attributes

- Extend the existing dot-by-dot PPU instead of replacing it.
- Depend on Slice 3 banking being complete before this slice: CGB PPU correctness requires real VRAM bank selection, tile attributes, and palette RAM to be modeled as hardware state instead of patched into the renderer.
- Implement this slice in internal substeps before treating `cgb-acid2` as acceptance: first the CGB PPU data model, then palette MMIO/blocking, then fetcher attribute consumption, then focused visual probes, then `cgb-acid2`, then Hacktix Bully hardening.
- Add BG tile-map attributes for VRAM bank, palette index, priority, horizontal flip, vertical flip, and writable/readable ignored bit `4`.
- Add CGB OBJ attribute handling for VRAM bank, OBJ palette index `0-7`, horizontal flip, vertical flip, OBJ-to-BG priority bit, color-index `0` transparency, and the distinction between CGB native OBJ palette selection and CGB compatibility `OBP0`/`OBP1` routing.
- Implement BG and OBJ palette RAM with `BCPS`/`BCPD` and `OCPS`/`OCPD`, including auto-increment and documented access blocking.
- Own the `FF40` / `LCDC.0` CGB priority reinterpretation in this slice: Non-CGB mode keeps the DMG BG/window enable meaning, CGB native mode uses it as BG/window master priority, and CGB compatibility mode must follow CGB-family priority/color semantics rather than enabling DMG silicon behavior.
- Own the object-priority algorithm through an explicit boot-latched object-priority mode that separates Non-CGB X-coordinate drawing priority from CGB OAM-order drawing priority; `OPRI` / `FF6C` post-boot writes may update the MMIO latch/readback, but they must not be treated as a visual runtime priority switch in ordinary Phase 10 unless a dedicated hardware-backed test proves the effect.
- Treat object selection, object drawing priority, and final BG-over-OBJ composition as separate PPU stages: the 10-objects-per-scanline selection rule remains OAM-scan based, the boot-latched object-priority mode selects the drawing-priority rule for overlapping OBJ pixels, and BG attributes / OAM priority / LCDC.0 decide the final BG-versus-OBJ result after the winning object pixel is chosen.
- In CGB native mode, the locked boot state should select CGB-style OAM-order object priority; in CGB compatibility mode, the locked boot state should select DMG-style X-coordinate object priority without enabling DMG-family silicon quirks such as OAM corruption.
- Keep DMG palettes (`BGP`, `OBP0`, `OBP1`), CGB native color palettes, and CGB compatibility palettes as distinct paths: Non-CGB mode renders through DMG grayscale palette registers, CGB native mode renders through CGB palette RAM selected by tile/OAM attributes, and CGB compatibility mode still renders through CGB palette RAM seeded by boot compatibility palettes while `BGP` indexes BG palette `0` and `OBP0`/`OBP1` index OBJ palettes `0`/`1`.
- Treat CGB compatibility palettes as a PPU mode policy, not as DMG silicon: the running mode may expose DMG-style palette registers to software, but the color output path, palette RAM ownership, boot-selected seed, and CGB-family quirk gates remain CGB-family state.
- Define the CGB compatibility palette seed contract before promoting compatibility-mode visual gates: `SkipBoot` must use the same standard CGB boot palette-selection algorithm as `RealBoot` for Nintendo-licensee detection, title checksum lookup, fourth-letter correction, lookup-index `0` fallback, and the runner-controlled boot-input override seam; the manifest should retain the resolved palette ID and the resulting BG/OBJ compatibility palette bytes for representative cases.
- Add focused CGB fetcher latch timing probes before broad visual acceptance: BG/window tile number and VRAM-bank-1 attributes must be sampled at the documented fetch boundary, palette index, flips, priority, and VRAM tile bank must remain stable for already fetched pixels, mid-scanline attribute or `VBK` writes must affect only later fetches according to the fetcher contract, and window start/restart must latch its own attribute-map entry instead of reusing stale BG attributes.
- Add synthetic tests before relying on broad visual ROMs: BG tile attribute decode, OBJ attribute decode, OBJ VRAM bank selection, BG and OBJ palette index selection, OBJ color `0` transparency, VRAM bank selection, palette RAM read/write, palette auto-increment, `BGPI`/`BGPD` blocking, `LCDC.0` CGB priority behavior, CGB compatibility palette seed injection, compatibility palette algorithm cases, `BGP` remapping through BG palette `0` in CGB compatibility mode, `OBP0`/`OBP1` remapping through OBJ palettes `0`/`1`, CGB native palette rendering ignoring DMG palette-register mappings, Non-CGB DMG grayscale remaining separate, `OPRI` read/write latch and boot-selected default, no ordinary post-boot `OPRI` visual priority mutation, Non-CGB X-coordinate OBJ priority, CGB OAM-order OBJ priority, CGB compatibility OBJ priority, BG-over-OBJ composition after object priority, horizontal flip, vertical flip, and CGB fetcher attribute latch timing.
- Treat `cgb-acid2` as the primary visual acceptance target, not as the first implementation driver; every visible fix must map to a documented CGB PPU primitive and ROM-specific visual patching is not allowed.
- CGB gate: `cgb-ppu-basic` through `make run-cgb-ppu-basic`, promoted in incremental order from palette-MMIO blocking and simple raster guards to `cgb-acid2` acceptance and Hacktix Bully hardening.
- Regression gate: DMG `167/167`, Acid DMG, and Mealybug DMG remain green.
- Status note: Slice `4A` palette-MMIO baseline is implemented for native CGB, Slice `4B` latches BG/window tile-map attributes from VRAM bank `1`, Slice `4C` consumes CGB OBJ attributes from live Mode `3` OAM metadata, Slice `4D` exposes the raw logical RGB555 framebuffer that looks up native CGB BG and OBJ palette RAM through the latched BG/OBJ palette-index sidebands while preserving OBJ color index `0` transparency, Slice `4E` implements the native CGB priority composer plus boot-latched OBJ priority mode and `OPRI` latch/readback baseline, Slice `4F` implements direct-boot CGB compatibility palette seed selection plus `BGP`/`OBP0`/`OBP1` routing through BG palette `0` and OBJ palettes `0`/`1`, Slice `4G` adds focused CGB fetcher latch timing probes for tile-index-dot attribute sampling, already-fetched pixel stability, next-fetch attribute visibility, `VBK` independence, and the four `cgb-ppu-basic` promotion rows now cover SameSuite `ppu/blocking_bgpi_increase.gb`, Daid `ppu_scanline_bgp.gb (GBC)`, Acid `cgb-acid2.gbc`, and Hacktix `bully.gb (GBC)` with `framebuffer-rgb555-fixture` oracles. The Hacktix row uses a narrow runner startup-timer profile for BullyGB's unconfirmed initial-`DIV` expectation while keeping the core Mooneye-backed CGB `SkipBoot` timer baseline unchanged, and its green path locks the CGB-family external-source OAM-DMA CPU policy that leaves internal WRAM/HRAM/MMIO accessible. With the Hacktix Bully visual promotion wired, no remaining `cgb-ppu-basic` row is listed for Slice `4` strict closure.

## Slice 5 — CGB DMA, GDMA, and HDMA

- Extend the existing DMA controller with CGB transfers instead of adding ad hoc bus branches.
- Keep this slice after the CGB PPU baseline because HDMA must observe LCD mode windows, HBlank boundaries, and the same bus visibility contract as the PPU.
- Extend the existing `FF46` OAM DMA path with CGB double-speed-aware duration and CPU blocking instead of treating OAM DMA as ordinary HDMA/GDMA or as a fixed DMG-only timing path; this slice consumes the speed-domain contract from Slice 2 but owns the DMA-controller-visible behavior.
- Add internal OAM DMA tests before promoting CGB DMA ROMs: start `FF46` in normal speed and double speed, assert copy correctness, first-byte commit timing, total transfer duration, CPU blocking window, HRAM accessibility during transfer, restart behavior, source-bus policy, and completion timing in both speed states.
- Add negative domain tests proving the CGB OAM DMA speed change does not imply a generic peripheral multiplier: LCD/PPU timing, HDMA block duration, and APU frame sequencing must retain their documented domains while OAM DMA follows the CGB speed-domain behavior.

### CGB OAM DMA bus arbitration matrix

This matrix is an internal core contract for Slice 5 and must be tested with synthetic HRAM-resident loops before CGB DMA ROM gates are promoted; DMG-family OAM DMA keeps the existing HRAM-only access rule, while CGB-family OAM DMA arbitrates by source bus.

| OAM DMA source bus | CPU access policy during CGB OAM DMA | required tests |
| --- | --- | --- |
| Cartridge bus source: ROM `0000-7FFF` or external RAM `A000-BFFF` | CPU access to WRAM and HRAM remains available; CPU access to the cartridge bus is blocked or returns the documented DMA-conflict value according to the shared bus policy | HRAM loop copies from cartridge source while reading/writing WRAM, plus a separate blocked cartridge-bus probe |
| WRAM bus source: WRAM `C000-DFFF` or Echo policy alias when accepted by the bus | CPU access to cartridge ROM/RAM and HRAM remains available; CPU access to WRAM is blocked or returns the documented DMA-conflict value according to the shared bus policy | HRAM loop copies from WRAM source while fetching/reading cartridge data, plus a separate blocked WRAM probe |
| Unsupported or edge source ranges | Behavior must be explicit and model-gated; do not silently fall back to DMG-wide blocking or invent a second DMA engine | Source-range tests for ignored, blocked, or documented-conflict behavior and retained bus traces |

- Model GDMA as a full-burst transfer.
- Model HDMA as block transfers advanced during the correct LCD window.
- Add internal `HDMA1-4` register tests for source and destination address masking, ignored low bits, destination confinement to VRAM, source policy for ROM/SRAM/WRAM versus unsupported or garbage-producing ranges, and preservation of latched transfer addresses across in-progress blocks.
- Define the active-HDMA bank-change policy before relying on SameSuite DMA ROMs: repo-gated tests may assume source ROM/RAM bank and `VBK` destination bank stay stable while HDMA is active, switching those banks mid-transfer is classified as unreliable/non-gating unless the transfer is paused or cancelled first, and any exploratory run that changes banks during active HDMA must retain per-block source bank, destination `VBK`, and bus-mapping traces instead of asserting a deterministic oracle.
- Add internal `HDMA5` state tests for start mode, length encoding, readback when inactive, readback while active, remaining-block count, cancellation by writing bit `7 = 0`, active/inactive transitions, and completion returning the documented inactive value.
- Add internal timing/window tests for HDMA: one block per eligible HBlank on lines `0-143`, no block advance during VBlank, LCD-off behavior, no accidental transfer when started in forbidden HBlank seams unless explicitly documented as project policy, destination overflow stopping behavior, CPU blocking during each block, and normal-speed versus double-speed timing where the hardware transfer duration stays in the documented VRAM-DMA domain rather than CPU M-cycle count.
- Add internal HDMA plus `HALT` tests: active HBlank DMA must pause while CPU execution is halted, resume only when the CPU resumes, preserve remaining-block readback while halted, and avoid leaking a second timing model into the CPU, PPU, or DMA controllers.
- Publish bus impact and CPU blocking through the shared scheduler/bus contract.
- CGB gate: `cgb-dma` through `make run-cgb-dma`.
- Regression gate: DMG `167/167` plus current DMG OAM DMA tests.
- Status note: Slice `5A` is started with a repo-owned exploratory `cgb-dma` manifest and `make run-cgb-dma` target covering the four initial SameSuite DMA rows as framebuffer informational captures, plus the first internal CGB OAM-DMA arbitration split where WRAM-source bursts publish a WRAM-bus-only CPU conflict instead of reusing the CGB external-source or DMG-family broad-block policy. Slice `5B` implements the `HDMA1-5` register surface: source/destination masking, CGB-only MMIO routing, `HDMA5` inactive/active readback, HBlank start latching, cancellation, and additive save-state defaults. Slice `5C` wires real GDMA/HDMA execution through the shared DMA work path: GDMA full-burst copies with CPU stall and VRAM bus occupation, HDMA one-block-per-window copies for visible HBlank or LCD-disabled windows, HALT pause/resume, explicit unsupported-source garbage, and destination overflow clipping. Slice `5D` locks CGB OAM DMA speed-domain behavior: `FF46` latches the current CGB speed profile, keeps the `160` CPU M-cycle OAM DMA body stable, exposes the double-speed LCD-domain dot-duration difference, preserves HRAM/source-bus arbitration and restart profile latching, and adds negative-domain tests proving LCD, HDMA, and APU timing do not inherit OAM-DMA speed handling. Bank-change traces, stricter HBlank seam edge policy, and promotion of blocking `cgb-dma` oracles remain open.

## Slice 6 — CGB-family boot ROMs

- Activate local validation for canonical CGB boot ROM assets through `GB_CYCLE_BOOT_ROM_ROOT`, using the canonical filename plus pinned hash as the strict contract for internal tests.
- Treat `cgb_boot.bin` as the Phase 10 default CGB boot ROM and reject mismatched assets in strict `RealBoot` validation by size and SHA-256 before execution.
- Keep `cgb0_boot.bin` and `cgbE_boot.bin` recognized as future revision-mode assets only: they may be hash-checked and listed in manifests, but they must not change Phase 10 behavior until CGB0/CGB-E revision modes are explicitly modeled.
- The `2304`-byte stored asset size is the repo-local padded CGB boot image layout that includes the unmapped cartridge-header gap at `0100-01FF`; the hardware-visible boot ROM bytes remain the Pan Docs `256 + 1792 = 2048` split across `0000-00FF` and `0200-08FF`, and the bus must not expose the padded gap as boot ROM data.
- Include standard CGB boot APU and wave-RAM closure in the centralized `SkipBoot` state: `cgb_boot.bin` must seed the same deterministic wave RAM bytes and relevant APU register/readback state that RealBoot produces, while the known `cgb0_boot.bin` wave-RAM difference remains a future revision-mode check rather than a Phase 10 behavior switch.

| boot asset | role | expected size | expected SHA-256 | Phase 10 policy |
| --- | --- | ---: | --- | --- |
| `cgb_boot.bin` | standard CGB boot ROM, default | `2304` bytes | `b4f2e416a35eef52cba161b159c7c8523a92594facb924b3ede0d722867c50c7` | Required for strict local CGB `RealBoot` closure |
| `cgb0_boot.bin` | early CGB0 boot ROM | `2304` bytes | `3a307a41689bee99a9a32ea021bf45136906c86b2e4f06c806738398e4f92e45` | Future revision-mode asset; not a Phase 10 default gate |
| `cgbE_boot.bin` | CGB-E boot ROM | `2304` bytes | `c56299bedd56debdbf36442238636bf5887a65c5173b33995682052353804da9` | Future revision-mode asset; not a Phase 10 default gate |

- Validate CGB boot ROM mapping windows at `0000-00FF` and `0200-08FF`.
- Own `FF50` as the CGB boot-overlay and handoff-lock boundary: the boot controller must expose the split CGB boot windows, keep the cartridge header visible where the boot ROM needs it, unmap boot ROM through the same one-way route as DMG-family boot, and lock the boot-selected `KEY0` state at the same handoff boundary.
- Compare real CGB boot handoff against the centralized CGB `SkipBoot` state.
- Keep this slice after CGB PPU and DMA bring-up so real boot validation compares against a coherent `SkipBoot` state rather than becoming a second source of truth for early CGB state.
- Own the RealBoot `KEY0` path: allow the CGB boot ROM to write `KEY0` before handoff, lock the resulting state when the boot overlay is disabled through `FF50`, and reject or ignore post-lock mutations according to the documented hardware model.
- Let real boot choose CGB native mode or CGB compatibility mode from the cartridge header through the same locked `KEY0` handoff state that `SkipBoot` synthesizes.
- Add CGB negative boot-path validation mirroring the DMG-family real-boot matrix but using CGB rules: valid header/logo/checksum reaches the `FF50` handoff, invalid logo data in the CGB-checked top half / first `$18` bytes fails, invalid data only in the unchecked lower half is handled according to executed CGB boot-ROM behavior rather than an emulator-side full-logo shortcut, invalid header checksum fails, all-`0xFF` or otherwise invalid headers remain in the boot ROM no-handoff path, and the emulator must not shortcut those checks outside executed boot firmware.
- Validate the locked boot-time `KEY0` / `OPRI` pair: CGB native handoff must preserve CGB-style object priority, while CGB compatibility handoff must preserve DMG-style object priority without reclassifying the machine as DMG silicon.
- Validate the boot-selected CGB compatibility palette seed against `SkipBoot`: RealBoot should populate the same CGB palette RAM compatibility colors for palette ID `0`, at least one known title-checksum table match, at least one fourth-letter correction case, representative DMG-only and dual-compatible headers, and any runner-controlled boot-input override cases promoted later; Slice 4 owns the runtime palette routing, while Slice 6 owns evidence that boot and direct boot start from the same compatibility palette state.
- Add a typed post-boot snapshot check for both `SkipBoot` and `RealBoot`, covering at minimum CPU registers, `KEY1`, `VBK`, `HDMA1-5`, `RP`, `BCPS`/`BCPD`, `OCPS`/`OCPD`, `SVBK`, `FF72-FF75`, `PCM12`/`PCM34` exposure policy, wave RAM, APU power/register readback state where deterministic, resolved compatibility palette ID, and any CGB-specific readback values that the direct-boot state seeds.
- Validate boot-derived visible memory state against `SkipBoot`: compatibility palette RAM, boot logo tile/tilemap seed when the selected compatibility path writes it, VRAM bank state, and any deterministic boot-owned VRAM/OAM/HRAM-visible artifacts promoted into the runner manifest must match RealBoot, while WRAM/HRAM bytes that Pan Docs treats as random or unreliable must use an explicit deterministic test policy without claiming hardware constancy.
- Add `cgb-boot-hwio` as the exploratory/internal Mooneye HWIO check for this slice; use `misc/boot_hwio-C.gb` when available, keep it out of strict blocking until the manifest records expected values and artifacts, and use it to detect divergence between RealBoot and the centralized `SkipBoot` handoff.
- CGB gate: `cgb-smoke` through `make run-cgb-smoke` and `cgb-boot-div` through `make run-cgb-boot-div` in standard `cgb_boot.bin` `RealBoot` mode plus a local `FF50` handoff smoke and negative no-handoff matrix; `cgb-boot-hwio` runs through `make run-cgb-boot-hwio` as exploratory/internal until promoted with documented HWIO expectations.
- Regression gate: existing real boot DMG0/DMG/MGB coverage plus DMG `167/167`.

## Slice 7 — CGB serial, APU, and timing regressions

- Do not implement a second CGB timer model in this slice; consume the Slice 2 timer/`DIV`/double-speed contract and add only regression tests for serial/APU behaviors that observe it.
- Implement CGB serial bits and speeds without changing DMG serial semantics: `SC.1` selects high-speed internal clock only when CGB serial features are enabled, while `SC.7` transfer enable and `SC.0` clock select keep the existing serial-controller ownership.
- Add internal CGB serial clock tests before relying on external ROM behavior: internal master transfers must run at `8192` Hz with `SC.1 = 0` in normal speed, `16384` Hz with `SC.1 = 0` in double speed, `262144` Hz with `SC.1 = 1` in normal speed, and `524288` Hz with `SC.1 = 1` in double speed, all derived from the shared speed-domain contract rather than hard-coded frontend timing.
- Add internal serial completion tests for CGB mode: live `SB` shifting, exact eighth-bit completion, `SC.7` clear timing, serial IRQ request timing, repeated transfers, mid-transfer speed-state invariants or documented rejection policy, and deterministic retained traces for failures.
- Add internal external-clock tests proving that externally clocked transfers still complete only on the eighth injected clock edge, accept irregular external clock spacing, preserve existing DMG external-clock behavior, and do not accidentally enable CGB high-speed semantics when `ConsoleModel` / `OperatingMode` does not expose them.
- After the single-machine serial contract is green, add a linked two-CGB validation manifest that runs one CGB as internal-clock master and one CGB as external-clock peer across normal-speed and double-speed cases, including high-speed `SC.1`, byte-completion ordering, per-participant serial artifacts, IRQ timing, disconnect/open-line behavior, and proof that the linked harness does not depend on desktop presentation loops.
- Mixed DMG↔CGB linked-session validation is intentionally out of Phase 10 and is a no-gate exploratory concern. Phase 10 validates DMG serial regression, CGB single-machine serial, and linked two-CGB behavior; mixed-model link sessions may be revisited only if a concrete commercial or test-ROM case requires it.
- Own only the `RP` / `FF56` register stub/readback baseline in Phase 10: expose documented CGB-only writable/read bits, emitter latch, disabled-receiver/no-signal readback, Non-CGB fallback, and boot-HWIO visibility, but do not implement host-side light injection, analog IR sensor adaptation, peer IR transport, or title-specific IR protocols in any Phase 10 slice.
- Adjust CGB APU defaults and timing, including double-speed interaction and explicit `PCM12` / `PCM34` register exposure when SameSuite APU cases are promoted.
- Build SameSuite APU coverage on shared channel primitives before chasing CH1/CH2 edge cases: frame sequencer, length counter, envelope, trigger, DAC, wave RAM access, and pulse-channel reuse must be reusable hardware components rather than ROM-specific patches.
- Require an APU shared-primitive checklist before promoting `cgb-audio-samesuite`: `FrameSequencer`, `LengthCounter`, `Envelope`, `Dac`, `Trigger`, shared `PulseChannelCore` for CH1/CH2, `Sweep` as a CH1-only extension, `WaveChannelCore` for CH3, `NoiseChannelCore` for CH4, and shared `DIV`/APU event scheduling.
- Add focused internal tests before fine SameSuite cases: CH1/CH2 pulse-core parity, CH1 sweep not duplicating CH2 pulse behavior, shared length/envelope/trigger/DAC behavior, centralized `DIV`/APU scheduling, and speed-change handling outside individual channel hacks.
- Treat SameSuite `div_*` rows as validation of the centralized `DIV` -> frame sequencer/APU event path, not as channel-local glitches; promote them only after natural `DIV` edges and `DIV` writes feed the same shared scheduler used by length, envelope, sweep, and channel trigger side effects.
- Add dedicated `DIV`/APU scheduler regression tests proving that natural `DIV` edges and `DIV` writes produced by the Slice 2 timer contract feed the same APU event path, CH1/CH2/CH3/CH4 subscribe to the shared scheduling source, and double speed does not create a separate frame-sequencer route; failures here must be fixed by the APU subscriber or by returning to Slice 2 if the underlying timer contract is wrong, not by adding a Slice 7 timer fork.
- SameSuite APU cases must not be fixed with channel-local hacks unless the behavior is genuinely channel-specific in hardware and documented at that channel boundary.
- Keep DMG-family OAM corruption gated by silicon family; CGB must not inherit it, including in CGB compatibility mode.
- CGB audio gate first: `cgb-audio-blargg` through `make run-cgb-audio-blargg`.
- CGB advanced audio gate: `cgb-audio-samesuite` through `make run-cgb-audio-samesuite`, promoted in coarse-to-fine implementation order: shared CH1/CH2 pulse-channel primitives plus coarse CH3/CH4 output, basic timing/frequency/restart behavior across channels, CH1 sweep, DIV/APU shared timing, fine channel-specific glitches, then speed-change-sensitive NRx2 cases.
- Regression gate: DMG `167/167` plus full Blargg `dmg_sound`.

## Slice 8 — CGB cartridge, RTC, and practical compatibility

- Validate that CGB mode does not regress existing MBC1, MBC2, MBC3, and MBC5 behavior.
- Keep `MBC30`, `MBC6`, and `MBC7` out of functional closure until the base CGB implementation is stable.
- CGB gate: `cgb-rtc` through `make run-cgb-rtc`.
- Regression gate: DMG `167/167` plus the current Phase 6 cartridge oracle suite.

## Slice 9 — CGB PPU hardening closure

- Use this slice for extreme mid-scanline write precision, not for initial CGB bring-up.
- Keep `cgb-acid-hell` as a closure signal only; it must not pull architectural bring-up work earlier than the basic PPU, DMA, boot, and timer/serial/audio slices.
- Add focused CGB palette-access microtiming probes before accepting `cgb-acid-hell` fixes: `BCPD`/`OCPD` data access during pixel transfer, index-register behavior separated from palette-data blocking, auto-increment or non-increment policy for blocked accesses, PPU-visible black-pixel or fallback-color artifacts where documented by Pan Docs/Gekkio/TCAGBD, and retained traces that identify the exact dot and LCD mode of the access.
- Add focused VRAM-access microtiming probes around Mode `2`/`3`/`0` seams: CPU VRAM read/write visibility, fetcher-visible bank and attribute snapshots, failed-write retention, HDMA versus CPU access ordering, and no broad Mode `3` shortcut that hides the pixel-fetcher dot responsible for the result.
- CGB final gate: `cgb-ppu-hard` through `make run-cgb-ppu-hard`.
- Promote this gate only after `cgb-ppu-basic`, `cgb-dma`, CGB boot, and CGB timer/serial/audio baseline gates are already green.
- Regression gate: DMG `167/167` plus SameBoy framebuffer differential when a stable comparable oracle exists.

## Post-Slice 10 — Out-of-scope CGB infrared expansion

- Full CGB infrared support is explicitly outside Phase 10 and must not block CGB bring-up, CGB PPU closure, DMA, boot, serial, APU, RTC, or `cgb-acid-hell`.
- Phase 10 implements only the `RP` / `FF56` register stub/readback baseline needed for documented CGB MMIO behavior and boot-HWIO checks.
- A future post-Slice 10 IR effort may add host-side light injection, sensor adaptation/fade behavior, peer IR transport, pulse timing, linked-session integration, title-specific workflows, and Dan Docs/Pan Docs cross-checks.
- Promote full IR only when there is a concrete validation target, such as a commercial title or dedicated IR test ROM, plus an explicit host-input/oracle artifact model; until then, keep IR as a documented hardware seam rather than speculative emulation.

## Post-Slice 10 — Out-of-scope PGB / PSM NMI research

- PGB mode, PSM NMI behavior, boot-ROM remap side effects after normal handoff, and undocumented `KEY0` / `OPRI` interactions beyond ordinary `$80` / `$C0` CGB behavior are explicitly outside Phase 10.
- Phase 10 must not silently enable PGB behavior from unusual header bits, direct `KEY0` values, or post-boot `OPRI` writes that would imply unverified visual priority switching; classify those cases as unsupported or experimental until hardware research, Pan Docs updates, and dedicated tests define a concrete model.
- Ordinary CGB native mode, CGB compatibility mode, `OPRI` boot defaults/readback, and boot-latched object-priority behavior remain Phase 10 scope; only the poorly researched PGB/PSM extension path and unverified post-boot `OPRI` visual effects are deferred.

## Post-Slice 10 — Out-of-scope AGB / GBA / GBP host behavior

- AGB/AGS/GBA and Game Boy Player host behavior is outside Phase 10; the Phase 10 CGB core must model CGB-family hardware, not a GBA running in CGB compatibility mode.
- Commercial smoke titles that mention GBA detection are reference-only sanity cases; their GBA-specific branches, display correction, host BIOS behavior, and Game Boy Player integration must not become CGB acceptance criteria.
- If a future AGB/GBP phase is opened, it must introduce explicit model axes, boot assets, post-boot register expectations, color-correction policy, and tests instead of overloading `ConsoleModel::GameBoyColor`.

## Cross-cutting CGB save-state and determinism rule

- Any slice that adds live CGB state must extend the typed whole-machine save-state contract before the slice is considered closed, following the Phase 8 rule that restore must capture hidden temporal state directly rather than reconstructing it from MMIO reads or replayed writes.
- Required CGB save-state coverage grows with the owning slice: Slice 2 adds speed/`KEY1`/timer hidden phase, Slice 3 adds VRAM/WRAM bank and CGB misc register state, Slice 4 adds palette/index/PPU priority state, Slice 5 adds GDMA/HDMA/OAM-DMA state, Slice 6 adds CGB boot handoff and locked `KEY0` state, Slice 7 adds serial CGB speed, `RP`, `PCM12`/`PCM34`, and APU subscriber state, and Slice 8 adds any CGB-mode cartridge/RTC validation state.
- Each promoted CGB suite should retain enough snapshot, trace, or replay metadata to reproduce the first post-restore divergence for the subsystem under test; save/load continuation failures must not be debugged by broad framebuffer or serial end-state comparison alone.

## Reference-only commercial CGB smoke list

These commercial titles are optional manual or manifest-driven smoke references only. They are not hardware oracles, they do not define pass/fail behavior, they must not block Phase 10 closure, and they must not be mixed into repo-gated ROM test counts; any local use must stay outside public CI and use non-redistributable ROM handling.

Practical header distinction: GB-compatible / CGB-enhanced dual-mode titles normally use CGB header `0x0143 = 0x80` and should run on both DMG and CGB, while CGB-only titles normally use `0x0143 = 0xC0` and require CGB-family hardware for correct execution.

### GB-compatible / CGB-enhanced smoke references

| game | useful to verify |
| --- | --- |
| Pokémon Gold / Silver | MBC3 plus RTC, SRAM/battery, DMG-to-CGB transition, saving, clock behavior, and IR only if implemented |
| The Legend of Zelda: Link’s Awakening DX | CGB palettes, maps/tilemaps, sprites, menus, and real dual-mode DMG/CGB behavior |
| Tetris DX | Stable baseline for input, audio, timers, save, and simple palettes |
| Wario Land II | Scrolling, sprites, priority, ROM/RAM banks, and DMG-versus-CGB comparison |
| R-Type DX | Horizontal scrolling, fast sprites, black-and-white mode, and color mode |
| Game & Watch Gallery 2 / 3 | Timers, input, audio, menus, multiple minigames, and compatibility routes |
| Dragon Warrior Monsters / Monsters 2 | Large RPG behavior: banking, SRAM, text, windows, menus, and long event paths |
| Pokémon Trading Card Game | Heavy menus, SRAM, optional link/IR, large text paths, and banked data |
| Pokémon Pinball | MBC5 with rumble, precise input, physics, audio, and SRAM |

### CGB-only smoke references

| game | useful to verify |
| --- | --- |
| Pokémon Crystal | CGB-only plus MBC3 RTC, SRAM, clock, IR, and regression comparison against Gold/Silver |
| The Legend of Zelda: Oracle of Ages / Seasons | MBC5, heavy PPU use, sprites, scrolling, windows, save, and GBA detection |
| Super Mario Bros. Deluxe | Precise scrolling, HUD/window behavior, input timing, menus, save, and CGB features |
| Wario Land 3 | VRAM restrictions during Mode `3`, KEY1 details, and broad CGB PPU behavior |
| Shantae | Strong stress case for scrolling, sprites, banking, and CGB/DMG APU differences |
| Alone in the Dark: The New Nightmare | Hi-color techniques and scanline palette changes; useful for PPU and palette timing |
| Metal Gear Solid / Ghost Babel | Large ROM banking, menus, scenes, audio, sprites, and scrolling |
| Donkey Kong Country | Visual streaming, tiles/palettes, scrolling, and PPU pressure |
| Perfect Dark | Rumble, IR, link/printer/Transfer Pak if supported, audio, and scenes |
| Kirby Tilt ’n’ Tumble | Tilt sensor / special cartridge input; only useful if cartridge-input hardware is planned |
| Warriors of Might and Magic | Mid-screen `SCX`/`SCY` changes and window behavior with `WX = 0` |
| Dr. Rin ni Kiitemite!: Koi no Rin Fuusui | Obscure double-speed / STOP and CGB speed-change behavior |

### Recommended smoke order

| order | category | games |
| ---: | --- | --- |
| 1 | Base CGB | Tetris DX, Link’s Awakening DX, Pokémon Gold / Silver |
| 2 | Mappers / save / RTC | Pokémon Crystal, Dragon Warrior Monsters, Pokémon Pinball |
| 3 | Serious CGB PPU | Oracle of Ages / Seasons, Wario Land 3, Donkey Kong Country |
| 4 | Accuracy / stress | Alone in the Dark, Shantae, Warriors of Might and Magic, Dr. Rin |
| 5 | Peripherals | Perfect Dark, Kirby Tilt ’n’ Tumble, Pokémon Trading Card Game |

## API and interface rules

- Keep `MachineConfig` as the single entry point for model, operating mode, host platform, and startup.
- Prefer `CapabilitySet` for behavior gates; use `ConsoleModel` only for silicon truth and `OperatingMode` only for software-visible mode truth.
- Add explicit state blocks rather than loose flags for speed state/`KEY1`, VRAM bank state, WRAM bank state, CGB palette RAM and index registers, boot-latched `OPRI` object-priority mode plus MMIO latch/readback, GDMA/HDMA transfer state, CGB serial speed state, `RP` infrared/external-port state, `PCM12`/`PCM34` APU output taps, undocumented CGB misc registers, and CGB boot handoff state.
- Do not create a second CGB core or parallel CPU/PPU/APU route.

## Test plan

- Every CGB PR or slice must run `make ci`, `make test-roms`, `make test-roms-cgb`, and the relevant `run-cgb-*` Makefile target for that slice after the target has fetched or verified its declared external ROM families.
- When a CGB suite is introduced, record its cases in a dedicated `crates/gb-test-runner/data/<suite>.toml` manifest and document its family, model, oracle, and `exploratory` versus `repo-gated` status before citing it as roadmap evidence.
- Promoting a CGB suite to repo-gated also requires a declared acceptance channel and retained artifacts; missing oracle-channel documentation blocks only promotion to blocking status, not exploratory execution or implementation progress.
- Do not add CGB suites to the default DMG gate.
- Treat `make test-roms` as the DMG gate even when the shared persisted report also contains rows from earlier CGB runs; CGB suite rows are produced only by `make test-roms-cgb` or the specific `run-cgb-*` targets.
- Use SameBoy as the primary oracle when a comparable artifact exists, and use GBEmulatorShootout to decide whether a ROM belongs in the repo-managed CGB catalog.
- Use `docboy-test-suite` as a late exploratory GBC precision oracle after the relevant CGB suites are already green; it must not become a blocking Phase 10 gate until each case has an explicit manifest entry, acceptance channel, retained artifact model, and source-of-truth mapping back to Pan Docs/Gekkio/TCAGBD.
- When a slice introduces new CGB live state, add focused save/load continuation or determinism coverage for that state before treating the slice as closed; this is a cross-cutting CGB gate, not a separate future cleanup phase.

## Assumptions

- SGB is out of scope for this roadmap.
- CGB revision variants are out of scope until the base CGB implementation is stable.
- AGB/AGS/GBA and Game Boy Player host behavior are out of scope for Phase 10 even when a reference-only smoke title contains GBA-detection logic.
- PGB mode, PSM NMI, and unusual undocumented `KEY0` flows are out of scope until a post-Slice 10 research effort defines dedicated hardware evidence and validation targets.
- `cgb-acid-hell` is a closure gate, not an initial architecture gate.
- ROM paths already passed on DMG may reappear as CGB-mode tests when they validate model/mode separation.
- Healthy progress means the active CGB slice is green and the DMG suite remains `167/167`.
