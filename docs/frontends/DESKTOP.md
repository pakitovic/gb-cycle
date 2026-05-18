# gb-desktop

SDL3-based desktop frontend.

```bash
cargo run --release -p gb-desktop -- [path/to/rom.gb]
```

Can start without a ROM and wait in a launcher-style root menu until a ROM is selected.

On macOS, `CAM LIVE` needs the process to run from an app bundle that declares `NSCameraUsageDescription`; otherwise the OS can leave SDL camera permission in `pending` without listing the binary under Privacy & Security. Use the development bundle launcher when testing Pocket Camera live input:

```bash
scripts/run-gb-desktop-macos-app.sh -- [path/to/rom.gb]
```

The launcher builds `gb-desktop`, creates `target/macos/GB Cycle.app` with the camera usage string from `crates/gb-desktop/macos/Info.plist`, ad-hoc signs it when `codesign` is available, and launches it through LaunchServices so macOS attributes camera permission to the `GB Cycle` bundle instead of the terminal or editor process that ran the script. Because LaunchServices detaches standard streams, this mode writes logs to `target/macos/gb-desktop.stdout.log` and `target/macos/gb-desktop.stderr.log`. macOS should then prompt for **GB Cycle** camera access, and the app appears under **System Settings -> Privacy & Security -> Camera**. If LaunchServices reports a launch error such as `-10810`, the script falls back to direct bundled execution; set `GB_CYCLE_DESKTOP_LAUNCH_MODE=direct` only when you explicitly want that fallback path for debugging.

For automation, local link startup, and reproducible profiling runs, the desktop CLI also supports:

- `--link-rom <path>` to start immediately with a linked secondary cartridge
- `--exit-after-frames <n>` to exit automatically after presenting `n` emulated frames
- `--no-rewind` to disable rewind capture for a profiling run without changing persisted menu settings
- `--test-runner` to use host-light automation defaults for screenshot and benchmark runners without changing emulated timing
- `--benchmark <path>` to load one portable benchmark TOML through `gb-benchmark`; the file owns the ROM path, model, startup/mode, screenshot/stat artifact toggles, and one or more fresh `[[run]]` entries. Each run starts a new desktop session, expands concise `[[run.input]]` pulses/repeats into deterministic joypad button edges, uses its duration as the final screenshot time, remains bounded by the matching T-cycle duration even if frame-origin progress freezes, and writes `gb-desktop/<artifact-id>.png` plus `gb-desktop/<artifact-id>-stats.toml` relative to the current benchmark directory when those toggles are enabled. Relative `rom` paths resolve against the TOML file directory, and `id` / `run.id` must use only ASCII letters, digits, `-`, and `_` because they form artifact filenames. The old top-level `duration_seconds` plus `[[stimulus]]` benchmark format is not accepted.

`scripts/run-benchmark.sh <case-dir>` is the portable local benchmark helper for desktop-first runs: it reads `*.toml` directly under `<case-dir>`, writes `gb-desktop/`, optional `gb-cli/`, and `index.html` under `scripts/benchmark/` next to the script, runs only `gb-desktop` by default, and adds CLI execution plus CLI report columns only when `--gb-cli` is present. Before launching either frontend, the helper resolves each case `rom` relative to the TOML file, skips cases whose ROM is missing, empty, or unreadable, and only runs benchmarks for the remaining valid cases so desktop does not show a ROM-read error dialog. For one case, `scripts/run-benchmark.sh --test path/to/game.toml` may omit `<case-dir>` because the helper infers it from the TOML path. Use `scripts/run-benchmark.sh --sample` to create a sample `scripts/game.toml` without creating a `test/` directory, use `scripts/run-benchmark.sh <case-dir> --rom-dir <rom-dir>` to rewrite each case `rom` path by preserving its basename under the supplied ROM directory, use `scripts/run-benchmark.sh <case-dir> --normalize-case` to rename existing case TOMLs to the exact ROM filename stem, and use `scripts/run-benchmark.sh <case-dir> --rom-dir <rom-dir> --generate-cases [--template <case.toml>]` to generate normalized case TOMLs for every `.gb` or `.gbc` ROM while inferring `model` from the ROM extension and keeping artifact `id` values safe.

Other host-side CLI overrides mirror the runtime menus and help output: `--save-dir`, `--save-key`, `--save-policy <manual|on-close|on-write|debounced>`, and `--no-saves` override cartridge persistence for the launched session; `--scale`, `--fullscreen`, `--no-vsync`, `--palette grey`, `--frame-blend <off|simple|lcd>`, `--mute`, and `--no-gamepad` override presentation/audio/input defaults; `--gamepad-direction`, `--gamepad-face-layout`, `--gamepad-bind-<up|down|left|right|a|b|select|start>`, `--gamepad-preferred-name`, and `--gamepad-preferred-path` override SDL gamepad routing before the first frame.

Local `DMG-07` 4-Player Adapter sessions are selected from the overlay at runtime through `EXT. PORT -> 4P ADAPTER`; desktop intentionally does not add a separate CLI shortcut for 3P/4P startup. Local CGB-to-CGB IR sessions are also overlay-only: with `SYSTEM -> MODEL GB COLOR` active, the root `CGB IR OFF` item appears below `EXT. PORT` and asks for a second ROM.

Use `--release` for normal gameplay and timing-sensitive validation. Unoptimized `debug` builds are intended for development and may run well below real-time on commercial games.

Use the workspace `release-max` profile when a slower compile/link step is acceptable for a more optimized desktop binary:

```bash
cargo run --profile release-max -p gb-desktop -- [path/to/rom.gb]
```

`release-max` enables fat LTO, a single codegen unit, symbol stripping, and abort-on-panic behavior. Keep plain `--release` as the default interactive build when shorter build times, portable diagnostics, or panic unwinding are more useful than final-binary optimization.

## Release packages

The GitHub release workflows package only the SDL3 desktop frontend. They all build `gb-desktop` with `cargo build --profile release-max -p gb-desktop --features gb-desktop/sdl3-build-from-source`, then run `gb-desktop --help` from the packaged runtime layout so the artifact proves that the executable can start with whatever SDL3 runtime form the source-build path emitted before upload.

The workflows live under `.github/workflows/`:

- `release-windows.yml` creates `gb-cycle-windows-x86_64.zip` on `windows-latest`, containing `gb-desktop.exe`, `README.md`, both license files, and every `.dll` emitted recursively by the source-build path under `target/release-max`.
- `release-linux.yml` creates `gb-cycle-linux-x86_64.tar.gz` on `ubuntu-latest`, containing `gb-desktop`, `README.md`, both license files, and any `libSDL3*.so*` found recursively under `target/release-max` and deduplicated by library filename; when a shared object is present, the binary is patched with an `$ORIGIN` rpath so it loads the bundled SDL3 shared object from the unpacked artifact directory without requiring `LD_LIBRARY_PATH`, and when no shared object is present the workflow fails if `ldd` still reports an SDL3 dynamic dependency.
- `release-macos.yml` creates only the Apple Silicon `gb-cycle-macos-aarch64.zip` package on `macos-latest`, containing `GB Cycle.app`, `README.md`, both license files, and any `libSDL3*.dylib` found recursively under `target/release-max` and deduplicated by library filename inside `Contents/MacOS`; when a dylib is present, the dylib install names are rewritten to `@executable_path` before ad-hoc signing, when no dylib is present the workflow fails if `otool -L` still reports an SDL3 dynamic dependency, and the bundle reuses `crates/gb-desktop/macos/Info.plist` so the release artifact carries the same Pocket Camera usage string as the local macOS launcher.

Manual `workflow_dispatch` runs are for artifact validation and upload the package through `actions/upload-artifact`. Tag pushes matching `v*` additionally attach the same package to the GitHub Release via `softprops/action-gh-release`; do not use a non-release branch tag unless you intentionally want to test the release-asset path and then clean up the temporary release/tag afterward.

## Core emulation

Reuses the same visible console model, startup mode, execution mode, boot-ROM search, and battery-save concepts as `gb-cli`. The desktop `--model` flag accepts only the hardware-profile names `DMG`, `MGB`, `LGB`, and `CGB`; the previous product names `game-boy`, `pocket`, `light`, and `color`, plus the legacy aliases `dmg0`, `dmg`, `mgb`, and `cgb`, are not accepted. `SYSTEM -> MODEL` selects the product model (`GAME BOY`, `GB POCKET`, `GB LIGHT`, `GB COLOR`), while `BOOT ROM -> ROM` selects the concrete firmware image used only by `RealBoot`; if the selected firmware is invalid for the model, the desktop configuration normalizes it back to that model's default.

`--mode` and `SYSTEM -> MODE` apply the complete `Strict`, `Permissive`, or `Experimental` compatibility preset, including validation, heuristic, override, and diagnostic policy, so desktop ROM loading follows the same cartridge-admission behavior as the automated runner for matching execution modes.

When a ROM is already loaded, `SYSTEM -> MODE` rebuilds the session under the next compatible preset. If the next preset in the cycle would reject the currently loaded cartridge metadata, desktop skips that preset, logs the skipped-mode reason to `stderr`, and tries the following preset instead of interrupting the menu flow with a modal cartridge-load error.

Host audio playback consumes a typed post-HPF sample-capture boundary from `gb-core`, so the desktop frontend only performs final host-side `f32` normalization and SDL3 queueing instead of owning APU semantics.

In CGB double-speed, desktop playback and recording capture that host-facing boundary only on the undoubled APU/LCD domain tick, not on every CPU-visible scheduler T-cycle, so a double-speed video frame enqueues roughly one frame of host audio instead of two.

## Audio investigation

- Use `--audio-record path/to/capture.wav` to export direct digital stereo APU output to `WAV` or `AIFC` without going through speakers, room acoustics, the macOS microphone path, or the frontend mute/volume controls.
- `--audio-record-rate <hz>` overrides the recording sink sample rate; the default is `96000` Hz so SameBoy-vs-`gb-cycle` commercial-ROM captures can be compared at the same host rate when desired.
- `--audio-record-stems <all|ch1,ch2,ch3,ch4>` writes isolated per-channel sidecar captures next to the main recording path (for example `capture.ch1.wav` and `capture.ch4.wav`) so commercial-ROM investigations can compare `CH1`, `CH2`, `CH3/WAVE`, and `CH4/NOISE` independently instead of only through the final mix.
- `AUDIO -> RECORD` in the desktop overlay starts/stops an automatic `WAV` capture at `96000` Hz under an `audios/` subdirectory next to the loaded ROM (for example `audios/zelda-0.wav`). That automatic recording uses the current desktop audio-channel selection instead of always forcing the full mix.
- `AUDIO -> CH1/CH2/CH3/CH4` are host-side diagnostic toggles. They do **not** rewrite `NR51` or any other APU register; they only change what the desktop frontend plays back or records.
- When all four channels are enabled, desktop playback and recording keep using the exact typed post-HPF `ApuHostSample` stream from `gb-core`. When a subset is selected, `gb-desktop` instead asks `gb-core` for the selected **pre-HPF** routed mix and then runs that mix through a dedicated host-side **T-cycle HPF state** before SDL playback or file encoding. That keeps the hardware model untouched while making solo/submix diagnosis practical under conditions that match SameBoy-style “mute other channels, then record the final mix” captures more closely than the old sample-rate DC-blocker path.
- Those per-channel stems start from the typed APU boundary **post-DAC / NR51 / NR50** and then pass through that same dedicated host-side **post-HPF** path before encoding. They are investigative host exports, not a claim that the hardware exposes an isolated solo-channel pin.
- Recording taps the same typed post-HPF `ApuHostSample` boundary that feeds SDL playback, but on an independent host sink, so it still works when normal desktop playback is muted or disabled for investigation.
- Fast Forward is a host playback mode: while held, the desktop frontend skips host frame-pacing sleeps, temporarily suppresses renderer vsync, does not capture/submit SDL playback audio, and clears the queue to avoid backlog, but `--audio-record` and `AUDIO -> RECORD` continue writing captured hardware audio through their independent recording sink.
- Set `GB_CYCLE_DESKTOP_AUDIO_LOG=1` to emit opt-in SDL audio telemetry to `stderr` with lightweight event logging only.
- Use `GB_CYCLE_DESKTOP_AUDIO_LOG=verbose` when you explicitly need one submit line per queued audio batch with queued-byte and queued-duration estimates.
- Set `GB_CYCLE_DESKTOP_AUDIO_DISABLE_AUTO_CLEAR=1` to disable the automatic oversized-queue recovery path entirely while investigating whether audible cuts come from host-side queue clears or from `gb-core`.
- Set `GB_CYCLE_DESKTOP_AUDIO_DISABLE_PACING_CORRECTION=1` to keep the same audio submit and queue behavior but remove the extra frame sleep derived from queued audio backlog, so host-side slowdown can be compared against the exact same `gb-desktop` build without switching branches.
- Both modes record stream pause/resume, queue clears, capture resets, mute, and volume changes so you can rule out host-side queue starvation or queue clears while reproducing a commercial-ROM audio issue in `gb-desktop`; they do not change APU timing or host audio policy.
- Steady-state frame pacing remains active even when renderer `vsync` is enabled, and applies a host-side correction from the current queued audio duration so normal gameplay converges toward roughly `100 ms` of queued audio instead of drifting upward when SDL presentation timing is loose.
- Automatic SDL queue clears remain a final emergency recovery path for oversized byte queues or repeated high-latency queues above roughly half a second, not the normal steady-state audio policy.

## Core trace capture

- Set `GB_CYCLE_DESKTOP_TRACE_PATH=/tmp/gb-desktop-trace.txt` to dump a rolling per-T-cycle CPU/APU/interrupt/joypad/bus trace when the frontend exits.
- Override the rolling window with `GB_CYCLE_DESKTOP_TRACE_T_CYCLES=<count>` when you need a narrower or wider final interaction window; the default is `8192` T-cycles.
- The rolling desktop trace also appends the cartridge trace summary from `gb-core`; for HuC1 investigations that includes the current mapper state plus `io_mode`, raw/effective ROM bank, raw/effective RAM bank, and the IR emitter / sensor bits.
- Cycles that include an APU MMIO write in `FF10..FF26` append `apu.last_write=... before(...) after(...)` so you can correlate audible glitches with the exact register transition that just occurred.
- Set `GB_CYCLE_DESKTOP_WATCH_TRACE_PATH=/tmp/gb-desktop-watch-trace.txt` together with `GB_CYCLE_DESKTOP_WATCH_TRACE_ADDRESSES=FF00,FF82,FF83,C400,C409,C41F` to dump a condensed rolling trace that only records T-cycles whose CPU bus activity or CPU address event touches one of those watched addresses.
- Override the watch-trace ring buffer size with `GB_CYCLE_DESKTOP_WATCH_TRACE_EVENTS=<count>`; the default is `4096` matched events, which is usually more useful than a full per-T-cycle window when a gameplay hang develops gradually before the final loop.
- Each watch-trace record includes the matching watched addresses, CPU bus/address info, `PPU` mode plus `LY`/`dot`, interrupt flags, live joypad state, and the cartridge trace summary, so commercial hang investigations can correlate stale cached state with mapper state and raster phase without wading through every intermediate T-cycle.
- Set `GB_CYCLE_DESKTOP_PC_WATCH_TRACE_PATH=/tmp/gb-desktop-pc-watch-trace.txt` together with `GB_CYCLE_DESKTOP_PC_WATCH_TRACE_RANGES=03C0-03EF,05FD-062F` to dump a second condensed rolling trace that only records T-cycles whose current `PC` falls inside one of those inclusive ranges. This is the right follow-up once an address watch has already shown stale state (for example `Pocket Bomberman` rereading `FF82`) and the next question is whether execution still reaches the expected refresh routine.
- Override the PC-watch ring buffer size with `GB_CYCLE_DESKTOP_PC_WATCH_TRACE_EVENTS=<count>`; the default is `4096` matched events. Single addresses are also accepted (for example `4C00`), and the range parser accepts `start-end`, `start..end`, or `start..=end` hex syntax.
- Each PC-watch record includes the matching PC range(s), CPU bus/address info, `PPU` mode plus `LY`/`dot`, interrupt flags, live joypad state, and the cartridge trace summary, so hang investigations can tell whether the game still revisits a suspected routine even when it has already stopped touching the raw MMIO addresses that routine normally refreshes.
- Set `GB_CYCLE_DESKTOP_EDGE_TRACE_PATH=/tmp/gb-desktop-edge-trace.txt` together with one or both of `GB_CYCLE_DESKTOP_EDGE_TRACE_ADDRESSES=FF82,C409,C400,C41F` and `GB_CYCLE_DESKTOP_EDGE_TRACE_PC_RANGES=4C00-4C4C` to dump an edge-triggered trace that records only **new** events: when the CPU enters one of the configured PC ranges from outside, or when a watched bus address is observed with a value different from the last observed value for that same address.
- Override the edge-trace ring buffer size with `GB_CYCLE_DESKTOP_EDGE_TRACE_EVENTS=<count>`; the default is `4096` triggered events. This is the best option once rolling watch traces are saturating with a stable loop and you need the exact moment a state machine flips into that loop.
- Each edge-trace record includes the current matching PC range(s), the entry/value-change trigger list, CPU bus/address info, `PPU` mode plus `LY`/`dot`, interrupt flags, live joypad state, and the cartridge trace summary. For `Pocket Bomberman`, this is the intended tool for catching when the bank-3 consumer first enters `0x4C00..0x4C4C` or when `FF82` / `C409` / `C400` change before the loop stabilizes.
- Set `GB_CYCLE_DESKTOP_CH4_NR43_TRACE_PATH=/tmp/gb-desktop-ch4-nr43-trace.txt` to dump a condensed event trace containing only live `NR43` writes plus the CH4 internal glitch/debug snapshot, which is more useful than the rolling trace when isolating Zelda-style tail-noise mismatches.
- Set `GB_CYCLE_DESKTOP_CH4_STARTUP_TRACE_PATH=/tmp/gb-desktop-ch4-startup-trace.txt` to dump CH4 startup events, including live writes to `NR52`, `NR42`, `NR43`, and `NR44` plus the moment a DMG delayed CH4 start fires.
- Set `GB_CYCLE_DESKTOP_CPU_WINDOW_TRACE_PATH=/tmp/gb-desktop-cpu-window-trace.txt` to dump the fixed legacy CPU/PPU window trace that starts when opcode fetch reaches `PC=0x4136` and stops after `PC=0x7A8F`; this is an investigation helper for a known execution window rather than a general-purpose trace filter.
- These trace captures are intended for interactive commercial-ROM investigation when reproducing the issue directly in `gb-desktop` is easier than scripting the same input path in `gb-test-runner`.
- When trace capture is disabled, the desktop loop skips the trace-record path entirely instead of calling into the ring-buffer helper on every `T-cycle`.

## Display and performance

- Opens a desktop window and renders the live `160x144` framebuffer for a single console. Local linked layouts render native panels side by side: `DMG-04`, CGB IR, and 2-player `DMG-07` use `320x144`; 3- and 4-player `DMG-07` use a `2x2` `320x288` grid, leaving the unused fourth panel black for 3P.
- Use `--palette grey` to force the startup desktop presentation palette to the same `DMG_GREY_DISPLAY_PALETTE` used by `VIDEO -> PALETTE GREY`; this override is applied only when the final effective `--model` is `DMG` and is ignored for `MGB`, `LGB`, and `CGB`.
- `VIDEO -> PALETTE` is a desktop-only DMG-family presentation selector with `PALETTE GREY`, `PALETTE GB`, `PALETTE POCKET`, and `PALETTE LIGHT`; it is enabled only for `GameBoy`, `GameBoyPocket`, and `GameBoyLight`, and it changes the live window and `VIDEO -> SCREENSHOT` output immediately without changing `gb-core`, CLI artifacts, ROM-test oracles, or rank-normalized test outputs.
- The model-aware default palette is `PALETTE GB` for `GameBoy`, `PALETTE POCKET` for `GameBoyPocket`, and `PALETTE LIGHT` for `GameBoyLight`; `GameBoyColor` disables the selector and renders the CGB RGB555 framebuffer directly, including GB-compatible software running on CGB silicon. `SYSTEM -> MODEL` resets the active desktop palette to the DMG-family default for the new model only after the model change has been applied successfully, and `VIDEO -> DEFAULTS` restores the default for the currently active model.
- `PALETTE GB`, `PALETTE POCKET`, and `PALETTE LIGHT` use SameBoy `Core/display.c` DMG/MGB/GBL RGB palettes in the desktop renderer and screenshots. The CGB model uses the core RGB555 framebuffer converted through the project’s deterministic 5-bit-to-8-bit profile instead of any Video Palette mapping; the legacy shade framebuffer remains for DMG presentation and debugging surfaces rather than CGB presentation.
- Host-side presentation filtering defaults to `OFF`, so the SDL texture is sampled with nearest-neighbor scaling unless `VIDEO -> FILTER` is enabled for linear smoothing.
- `VIDEO -> BLEND` is a desktop-only LCD persistence simulator with `BLEND OFF` (default), `BLEND SIMPLE`, and `BLEND LCD`; `SIMPLE` gamma-blends the current RGB24 frame 50/50 with the previous raw frame, while `LCD` uses the SameBoy-style gamma blend with previous-frame weight alternating between `1/3` and `2/3` by scanline parity and presented-frame parity. The blend is applied after DMG palette or CGB RGB555 conversion and before HUD, rewind/Fast Forward indicators, and menu overlays; it never changes `gb-core` timing, framebuffers, save states, CLI oracles, or host texture filtering.
- `VIDEO -> BACKGROUND`, `VIDEO -> WINDOW`, and `VIDEO -> OBJECTS` are DMG-family debug presentation masks that do not touch core timing or `LCDC` state. With all masks enabled, the live window and screenshots present the core's final visible framebuffer directly for BG/WIN pixels instead of substituting the diagnostic BG/WIN sidebuffer. Disabling `OBJECTS` reveals the stored BG/WIN plane underneath in both the live window and screenshots; if `BACKGROUND` and/or `WINDOW` are also masked away, the uncovered area falls back to the per-pixel DMG backdrop shade (palette entry `0` under the historical `BGP` value) instead of a fixed solid fill, so OBJ-only captures track SameBoy's changing diagnostic backdrop more closely. CGB presentation renders the final RGB555 framebuffer directly rather than trying to synthesize layer-masked color planes without dedicated RGB555 layer sidebuffers.
- `VIDEO -> SCREENSHOT` saves a native-size PNG next to the running ROM inside a `screenshots/` subdirectory using an `8-bit RGB` layout similar to SameBoy’s raw screenshots, without baking in host-side scaling, filtering, frame blending, HUD, or menu overlays.
- The live window presents immediately when the CPU enters `STOP` / `ZombieStopped` and the core forces visible blank output, even if no later frame boundary can occur; this keeps `STOP` diagnostics from leaving a stale pre-STOP SDL texture onscreen while preserving the framebuffer used by `VIDEO -> SCREENSHOT`.
- Window title reports live FPS, average frame time, relative emulation speed, and a frontend-side breakdown of average emulation, render, pacing, and audio-queue timing.
- Compact in-window performance HUD with those same frontend metrics; it defaults to `OFF` and remains toggleable from `VIDEO` or through a dedicated remappable hotkey.
- Set `GB_CYCLE_DESKTOP_EMU_PROFILE=1` or `GB_CYCLE_DESKTOP_EMU_PROFILE=summary` to emit opt-in sampled `EMU` breakdowns to `stderr`; these aliases preserve the full profiler detail and report `profile_detail=full`.
- The default/full mode replays one cloned start-of-frame state every `15` presented frames on a background worker, then reports sampled averages for the real measured frame time plus normalized `gb-core` estimates for `CPU`, `PPU`, and the remaining core buckets split into external-event ingress, timer, APU, DMA, serial, and interrupt handling; the serial bucket also carries cheap replay counters for active serial T-cycles, internal/external transfer ticks, external wait ticks, shift edges, completed bytes, and external-port ticks so link activity can be separated from observer cost without adding serial `Instant` sub-buckets.
- The sampled `PPU` bucket is further split into `mode0_1`, `mode2`, `mode3_startup`, background fetch, window fetch/restart, BG push/fill, OBJ fetch, pixel transfer, PPU bus-sync boilerplate, debug/test owner bus-state validation, PPU bus-view acquisition, PPU owner snapshot, published CPU access, tick guard/dispatch, Mode 3 control dispatch, BG/window/OBJ edge work, raster publication, mode timing, raster advance, STAT IRQ refresh, visible-line prep, PPU miscellaneous work, `ppu_other`, duplicate `ppu_unbucketed`, and duplicate `ppu_profile_gap` remainder fields so menu and HUD slowdowns can be narrowed to a specific raster phase without instrumenting the main thread.
- Use `GB_CYCLE_DESKTOP_EMU_PROFILE=summary-lite[:frames]` or `lite[:frames]` for `profile_detail=core`, which keeps the cloned replay and outer machine regions (`ppu_ms`, `cpu_ms`, `core_other_ms`) but disables PPU sub-region observer callbacks; the PPU sub-buckets, `ppu_other`, `ppu_unbucketed`, and `ppu_profile_gap` remain present for stable log parsing but are not valid semantic splits in this mode.
- Use `GB_CYCLE_DESKTOP_EMU_PROFILE=summary-overhead[:frames]` or `overhead[:frames]` for `profile_detail=overhead`, which replays the same cloned frame start three ways (`step_t_cycle`, core-only observer, full observer) and emits `profile_base_ms`, `profile_core_ms`, `profile_full_ms`, `profile_core_overhead_ms`, and `profile_ppu_observer_overhead_ms` so observer instrumentation can be separated from emulator core cost.
- Coarse frontend work that still lives inside the measured emulation window remains reported from the real frame (`SDL` event polling, audio submit, save flush), and the summary also emits sampled `serial_active_tcycles`, `serial_internal_ticks`, `serial_external_ticks`, `serial_wait_external_ticks`, `serial_shift_edges`, `serial_completed_bytes`, `serial_ext_port_ticks`, `frame_tcycles`, `scheduler_tcycles`, `video_dots`, `speed_mode`, `frame_start_ly`, `frame_start_dot`, `frame_end_ly`, `frame_end_dot`, `frame_crossings`, `scanline_transitions`, `scanlines_over_456`, `max_scanline_tcycles`, `max_scanline_ly`, `max_mode0_start_dot`, `max_mode0_start_dot_ly`, `ly153_to0`, `ly153_to0_startup`, `ly153_to0_blank`, `ly0_self_wraps`, `ly0_self_wrap_startup`, `ly0_self_wrap_blank`, `ly0_to1`, `ly0_tcycles`, `ly0_max_mode0_start_dot`, `ly0_stall_tcycles`, `ly0_stall_hb_tcycles`, `ly0_stall_oam_tcycles`, `ly0_stall_draw_tcycles`, `ly0_stall_startup_tcycles`, `ly0_stall_blank_tcycles`, `ly0_stall_runs`, `ly0_max_stall_tcycles`, `ly0_max_stall_dot`, `ly0_max_stall_mode_dot`, `cpu_stop_tcycles`, `cpu_zstop_tcycles`, `ly0_stop_tcycles`, `ly0_zstop_tcycles`, `ly0_stall_stop_tcycles`, `ly0_stall_zstop_tcycles`, `lcdoff_tcycles`, `lcdoff_transitions`, `lcdon_transitions`, `ly0_lcdoff_tcycles`, `ly0_stall_lcdoff_tcycles`, `submit_samples`, `submit_tcycles`, `submit_queue_before_ms`, `submit_enqueued_ms`, `submit_queue_after_ms`, `audio_queue_before_ms`, and `audio_queue_after_ms` plus host-side `present_ms`, `pac_ms`, `sleep_target_ms`, `audio_corr_ms`, `late_ms`, and `oversleep_ms` so compositor or pacing jitter can be separated from core cost and correlated with backlog-driven audio correction, including direct `LY=0` stall detection at the frame boundary, whether it overlaps `STOP`/`ZombieStopped`, and whether the PPU actually enters LCD-off state inside the bad frame.
- When `serial_ms` is non-zero with `serial_external_ticks>0`, `serial_wait_external_ticks>0`, and `serial_shift_edges=0`, the serial bucket is observing an external-clock wait state rather than actual shifted link bytes or Printer traffic.
- Summary lines also tag the active session shape as `session=single`, `session=linked-dmg04-2p`, `session=linked-cgb-ir-2p`, or `session=linked-dmg07`, so single-console and linked runs can be compared mechanically from the same profiler output stream.
- The detailed frame-boundary, scanline, `LY=0` stall counters, machine-step region callbacks, and PPU sub-region classification are only collected while an observer/profiler requests them, so normal desktop gameplay does not pay that extra per-`T-cycle` bookkeeping cost.
- Linked-session profile replays also use the linked observer stepping path, so sampled `CPU`, `PPU`, and `core_other` buckets remain populated for local `DMG-04` runs instead of collapsing to zero during the background replay.
- When audio playback/recording, rewind capture, printer output, gamepad rumble, and desktop trace captures are inactive, the desktop loop skips those frontend-only per-`T-cycle` hooks entirely while leaving core stepping, host RTC synchronization, and frame pacing unchanged.
- `--test-runner` additionally skips menu/hotkey/keyboard processing, pending native-dialog work, title/stat updates, and in-frame SDL event polling; only a lightweight quit-event drain runs once per presented frame, plus once-per-frame joypad polling when gamepad support is explicitly re-enabled, so automated screenshot runners avoid desktop host work while `Machine::step_t_cycle()`, `HostRtcSync`, and `FramePacer` stay on the normal path.
- Normal gameplay also skips repeated rumble synchronization work for cartridges without rumble support unless the frontend still has an applied host rumble effect to clear.
- Override the sampling stride with `GB_CYCLE_DESKTOP_EMU_PROFILE=summary:<frames>`, `summary-lite:<frames>`, or `summary-overhead:<frames>` when you need denser or lighter sampling during an investigation.
- That profiler is investigative timing only: it does not alter emulation semantics, it is disabled by default, and it is designed to minimize main-thread intrusion while still separating likely core cost from host overhead when a commercial ROM path drops below full speed.

For reproducible single-vs-linked desktop profiling comparisons, use the same release build and SDL dummy drivers for both runs so the profiler compares host overhead and linked-session cost under the same conditions:

```bash
SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy \
GB_CYCLE_DESKTOP_EMU_PROFILE=summary:30 \
cargo run --release -p gb-desktop -- /path/to/tetris.gb \
  --mute --no-gamepad --no-vsync --exit-after-frames 180

SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy \
GB_CYCLE_DESKTOP_EMU_PROFILE=summary:30 \
cargo run --release -p gb-desktop -- /path/to/tetris.gb \
  --link-rom /path/to/tetris.gb \
  --mute --no-gamepad --no-vsync --exit-after-frames 180
```

## Input

- Maps keyboard and SDL3 gamepad input to the joypad path.
- Clamps impossible stock-handheld direction pairs (`Left+Right`, `Up+Down`) to neutral before they reach the core joypad state, even when they come from mixed host sources such as keyboard + gamepad or D-pad + left stick.
- Basic gamepad hotplug plus preferred-device selection and remappable gamepad bindings.
- Shows the current active gamepad in the `GAMEPAD` submenu; can pin or clear the preferred device from that UI.
- Can move gamepad focus to the last used controller whenever no preferred device is currently locked.
- Local multi-console sessions route host input through frontend-owned player slots. `P1` keeps the configurable keyboard/gamepad profile. Its default keyboard joypad profile uses arrow keys, `Left Option` for `B`, `Left Command` for `A`, `Backspace` for `SELECT`, and `Enter` for `START`. The keyboard menu defaults mirror those face buttons with `Left Command` as confirm/`A` and `Left Option` as cancel/`B`; `Esc` also remains a hardwired cancel shortcut. Default hotkeys use `Space` for pause, `F1` for save state, `F2` for load state, `1`/`2`/`3`/`4` for selecting the active state slot, `Left Shift` for rewind, `Right Shift` for Fast Forward, `F9` for manual cartridge save, `F10` for stats, `F11` for fullscreen, and `F12` for reset.
- Local `DMG-04` and CGB IR sessions use the explicit `P2` keyboard profile for the secondary console. `DMG-07` reuses that `P2` profile and adds fixed keyboard profiles for `P3` and `P4`.
- Fixed linked-player keyboard profiles:
  - `P2`: `WASD` directions, `Z/X` for `B/A`, `Q/E` for `SELECT/START`
  - `P3`: `TFGH` directions, `V/B` for `B/A`, `R/Y` for `SELECT/START`
  - `P4`: `IJKL` directions, `M/,` for `B/A`, `U/O` for `SELECT/START`
- Gamepad input remains assigned to `P1` only.

### Rebinding

All rebinding takes immediate runtime effect:

- `INPUT -> KEYBOARD` — in-window keyboard joypad rebinding.
- `INPUT -> KB MENU` — dedicated host-side keyboard menu rebinding.
- `INPUT -> HOTKEYS` — frontend hotkey rebinding.
- Keyboard rebinding uses SDL3 physical scancodes when available so saved bindings stay stable across host layouts. Supported keyboard keys include the existing arrows, top-row `1`/`2`/`3`/`4`, `Backspace`, `Enter`, `Space`, `R`, `X`, `Z`, all function keys from `F1` through `F12`, plus `Tab`, left/right `Shift`, left/right `Control`, left/right `Alt` (`Option` on macOS), and left/right GUI (`Command` on macOS, Windows/Super on Windows/Linux). `Fn` remains host/firmware-owned and is not treated as a reliable bindable key.
- `INPUT -> GAMEPAD` — SDL gamepad rebinding, including standard SDL buttons plus the analog `L2`/`R2` trigger axes treated as digital bindings at the desktop input boundary.
- `INPUT -> GAMEPAD` also exposes host-action bindings for `SAVE STATE`, `LOAD STATE`, `REWIND`, and `F-FORWARD`. These action bindings are independent from joypad buttons, are persisted with the active gamepad settings, and default to `NONE` so the stock controller profile is not changed until the user opts in. Hold-style host actions are cleared when the active SDL gamepad changes or disconnects, so a missing `ControllerButtonUp` or trigger-axis release event cannot leave rewind or Fast Forward stuck on.
- `INPUT -> PAD MENU` — dedicated SDL gamepad menu rebinding.
- `INPUT -> RUMBLE` — host rumble mode for the active SDL gamepad with `OFF`, `HIGH`, and `LOW` host-intensity options. This option is only enabled when the loaded cartridge exposes rumble support and the active gamepad reports SDL rumble capability; otherwise it remains visible but disabled.
- `INPUT -> GYRO` — host MBC7 tilt source with `OFF`, `PAD GYRO`, and `PAD INPUT` modes. This option is visible but disabled unless the loaded cartridge exposes the MBC7 accelerometer and an SDL gamepad is active; `PAD GYRO` reads the active gamepad's SDL accelerometer, auto-centers when the mode is enabled or the active gamepad changes, and sends the calibrated acceleration delta to MBC7, while `PAD INPUT` maps the right stick directly to a virtual ±1000 mg tilt with right/down positive and no extra deadzone. Despite the UI label, the MBC7 runtime receives accelerometer/tilt input because MBC7 models ADXL202E acceleration rather than angular velocity.

## Overlay and menus

Pause/menu overlay with native SDL3 `Open ROM` filtered to common Game Boy ROM extensions, plus frontend-owned submenus:

- `Escape` and the active gamepad `Guide` button open the overlay when it is closed; when it is already open they both act as the same back/cancel control.
- In launcher mode without a loaded ROM, that shared back/cancel behavior does not dismiss the root overlay.
- While a native file dialog is pending from the overlay, the triggering entry stays selected but disabled until the dialog resolves.
- Once a ROM is loaded, the root overlay exposes single-machine `.gbstate` v1 actions immediately below `OPEN RECENT`: `SAVE STATE`, `LOAD STATE`, `STATE SLOT N`, and `AUTOLOAD OFF` / `AUTOLOAD SLOT N`. They are hidden in the launcher/no-ROM root menu to keep startup uncluttered; after a ROM is loaded, `LOAD STATE` remains visible but disabled until the selected slot file exists. The autoload selector is persisted, cycles through `OFF` and slots `1` through `4`, and attempts to restore the selected slot after `OPEN ROM` / `OPEN RECENT` only when that slot file exists; missing slots are ignored without a modal error. Hotkey and gamepad `LOAD STATE` actions also no-op without a modal error when the selected slot file is absent, while corrupt or unreadable existing state files still report an error. State defaults are `F1` save, `F2` load, and `1`/`2`/`3`/`4` for selecting the active slot. Rewind is controlled by the `Left Shift` hold hotkey rather than a one-step root menu action and, by default, consumes four older in-memory snapshots per presented frame while held. Fast Forward is controlled by the `Right Shift` hold hotkey by default and uses the retuned `2x`/`4x`/`8x` presets from `SYSTEM -> F-FORWARD`.
- `SYSTEM -> REWIND` owns desktop-only rewind policy: `REWIND ON/OFF`, `HISTORY` (`5`/`10`/`20`/`30`/`60` seconds), `SUBFR` (`OFF`/`1`/`2`/`4` subframes per frame), `SPEED` (`1x`/`2x`/`4x`, mapped to `2`/`4`/`8` snapshot restores per presented frame while held), `MEMORY` (`64`/`128`/`256`/`512` MiB), and `DEFAULTS`. Capture-policy changes rebuild the in-memory buffer and clear history so old snapshots are not mixed with a different cadence or capacity; changing `SPEED` only affects playback and keeps the existing history.
- `SYSTEM -> F-FORWARD` owns the desktop-only Fast Forward policy: `F-FORWARD ON/OFF`, `SPEED` (`2x`/`4x`/`8x`, default `2x`), and `DEFAULTS`. The visible `2x`, `4x`, and `8x` presets map to internal speed multipliers `4`, `8`, and `16` respectively. `ON/OFF` controls availability only: when it is `ON`, holding `Right Shift` or the mapped gamepad Fast Forward action accelerates; when it is `OFF`, those associated inputs do nothing. Fast Forward does not change `gb-core` timing, does not write into save states, and only changes desktop host frame pacing, renderer vsync while held, presentation, playback-audio submission, and rewind-history capture.
- The compact stats HUD includes rewind status next to the frame/audio metrics: `RW OFF` when disabled or linked, `RW EMPTY` with no history, `RW <seconds>S <count>` when history is available, `RW << <seconds>S` while actively rewinding, and `MEM <used>/<limit>M` using core-accounted snapshot payload bytes. Holding rewind with an empty buffer does not show a modal error and does not advance emulation during that frame. While the rewind hotkey is active, a separate top-right `<< REW` indicator is rendered even when the stats HUD is hidden; it is suppressed when rewind is off or unsupported.
- While Fast Forward is active, a separate top-right `FF >>` indicator is rendered even when the stats HUD is hidden. If rewind and Fast Forward are both held, rewind wins and the rewind indicator is shown instead.
- `.gbstate` slots and rewind are disabled by design in local `DMG-04` 2-player Game Link, CGB IR, and `DMG-07` 4-Player Adapter linked sessions; save states and rewind remain single-machine features.
- Root overlay also exposes `QUIT` directly at the first menu level.
- Root-level back/cancel (`Escape` / `Guide`) clears an explicit manual `SPACE` pause before closing the overlay, and loading a new primary ROM from `OPEN ROM` / `OPEN RECENT` also leaves the frontend unpaused so screenshot/debug workflows do not strand the session in a hidden paused state.
- When the loaded session includes a `Pocket Camera` cartridge, the root overlay also exposes:
  - `CAM LIVE ON` / `CAM LIVE OFF` — opens or stops the first SDL3 camera device with SDL's native stream selection, converts each available frame to grayscale, mirrors it horizontally for self-facing Pocket Camera orientation, and pushes it through the same core API used by `CAM IMAGE`
  - `CAM IMAGE` — native PNG picker that decodes the selected image in the frontend and pushes it into the core as a grayscale host frame
  - `CAM RESET` — stops live capture if active, clears the current session image, and restores the core's deterministic placeholder frame
- These entries appear before the general ROM/system menus so Camera ROM sessions expose live capture state first.
- Pocket Camera still-image selection and live-camera state are session-scoped only. A chosen still image is reapplied across ROM reloads / resets while the desktop app stays open, but neither still-image path nor live-camera state is persisted into desktop settings.
- Camera permission, device selection, native frame acquisition, RGB conversion, horizontal live-frame mirroring, and warm-up frame dropping are frontend-owned. `gb-core` only receives grayscale host frames and performs the deterministic `128x112` normalization.
- If SDL opens a camera but no frames arrive, the desktop log reports whether SDL still considers camera permission `pending`, `approved`, or `denied`; this keeps OS permission stalls distinguishable from frame acquisition stalls.
- **`VIDEO`** — stats HUD visibility (default `OFF`), host-side presentation filter, LCD frame blending, desktop palette selection, fullscreen, vsync, window scale, integer presentation, screenshot capture, and BG/WIN/OBJ presentation masks.
- **`AUDIO`** — toggle mute, cycle host volume, host-mask `CH1..CH4`, and start/stop automatic `WAV` captures under `audios/`.
- **`INPUT`** — keyboard, gamepad, hotkey, and menu rebinding (see above).
- **`EXT. PORT`** — `NONE`, `PRINTER`, `GAME LINK`, and `4P ADAPTER`. `GAME LINK` keeps the real two-cartridge `DMG-04` flow and asks for a second ROM. `4P ADAPTER` opens a `2 PLAYERS` / `3 PLAYERS` / `4 PLAYERS` submenu; selecting a count rebuilds a fresh local `DMG-07` session and clones the already-loaded `P1` ROM into every adapter slot instead of opening more ROM dialogs.
- **`CGB IR OFF` / `CGB IR ON`** — root-level CGB-to-CGB infrared pairing, visible only when `SYSTEM -> MODEL GB COLOR` is selected. When off, selecting `CGB IR OFF` asks for a second ROM and rebuilds a fresh two-console native-CGB optical pair. When active, selecting `CGB IR ON` turns the pair off, returns to the single primary console without opening the ROM picker, and the root item goes back to `CGB IR OFF`. It keeps serial / `EXT. PORT` attachments at `NONE`. This intentionally supports different cartridges in the two slots, such as Pokémon Gold / Silver / Crystal combinations, without treating IR as a cable-link mode.
- **`SYSTEM`** — system-level options such as console model, startup mode, execution mode, the `BOOT ROM`, `SAVE`, `REWIND`, and `F-FORWARD` submenus, and reset.
- **`SYSTEM -> BOOT ROM`** — boot-ROM-specific options: `BOOT AUTO`, `BOOT FILE`, `BOOT DIR`, and `VERIFY`. `MODEL`, `START`, and `SAVE` remain at the system/save level so hardware model selection, startup policy, and cartridge persistence are not mixed with boot-ROM asset configuration.
- **`SYSTEM -> SAVE`** — save-specific options: `EXPORT SAVE`, `IMPORT SAVE`, `SAVE BATTERY`, `SAVES ON/OFF`, `SAVE POLICY`, `DIR AUTO`, and `SAVE DIR`. This keeps cartridge persistence controls in one submenu instead of mixing them into the root or top-level system option list.
- **`OPEN RECENT`** — recent-ROM history for the last `12` ROMs, available from the root overlay whenever recent ROMs exist; entries can relaunch directly, the submenu exposes `CLEAR LIST`, and the selected entry scrolls after a short dwell when the sanitized title is wider than the overlay text area.
- **`DEFAULTS`** — reset actions inside `VIDEO`, `AUDIO`, and `INPUT` to restore host-side settings and bindings without touching CLI config.

## Battery saves

- Default policy: debounced auto-flush — once cartridge persistence changes, the frontend writes a safe replacement save after roughly `2s`, and forces a flush on ROM changes and shutdown.
- Default internal save filenames preserve the active ROM's exact filename stem: `Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb` maps to `Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gbsav` for P1. The same stem is used for the default external `.sav` export/import filename. The desktop frontend does not probe older sanitized save names; rename old files manually when needed.
- For RTC-backed `MBC3` cartridges, the desktop loop derives a `32.768 kHz` RTC tick budget from suspend-aware host wall-clock elapsed time and releases pending ticks on the active emulation T-cycle cadence instead of batching them at host event or frame boundaries; lifecycle actions such as save export flush any pending ticks before serializing persistence. This preserves MBC3 subsecond phase while clock-based games keep advancing as the ROM remains open, including across host sleep/resume, instead of only catching up on the next save reload.
- `SYSTEM -> SAVE -> EXPORT SAVE` writes the current primary/P1 cartridge persistence as a SameBoy/mGBA-compatible `.sav`. The native save dialog defaults under `saves/export` next to the active ROM or configured save root; if the chosen output path has no extension, the frontend appends `.sav`.
- `SYSTEM -> SAVE -> IMPORT SAVE` reads the selected external save path as-is (the dialog defaults to `.sav`, `.sa1`, `.sa2`, `.sa3`, or `.sa4`, but extensionless files are valid), validates it against the current primary/P1 ROM, writes the matching internal `.gbsav`, and then asks the user to reload or reset the game. Import does not hot-swap the live cartridge session; after a successful import the active primary save session is disabled until reload so the running game cannot overwrite the imported `.gbsav`.
- The external `.sav` compatibility boundary mirrors the CLI converter: linear cartridge RAM is raw bytes, `MBC3` RTC saves export the shared `48`-byte suffix and import both the `44`-byte and `48`-byte suffix forms, and `MBC2` import accepts SameBoy and mGBA layouts while export defaults to mGBA packed bytes.
- Local multi-cartridge sessions isolate battery saves by player-slot file extension while keeping the ROM stem unchanged: P1 uses `<stem>.gbsav`, P2 uses `<stem>.gbsa2`, P3 uses `<stem>.gbsa3`, and P4 uses `<stem>.gbsa4`.
- In local `DMG-07` sessions, each visible player slot models a separate cartridge instance even when the ROM bytes are cloned from `P1`; those cloned instances use the slot extension for the active player. In local CGB IR sessions, `P1` uses the primary ROM stem with `.gbsav` and `P2` uses the selected secondary ROM stem with `.gbsa2`, so different IR cartridges stay isolated without topology-specific suffixes.
- The `SAVE BATTERY` menu action is only exposed inside `SYSTEM -> SAVE` when the desktop save policy is explicitly set to `manual`.

## Error handling

User-facing desktop failures such as ROM open/load errors surface through native SDL3 message boxes instead of only writing to `stderr`; technical diagnostics remain in terminal output. `SYSTEM -> START SKIP`, `SYSTEM -> START CUSTOM`, and `SYSTEM -> START REAL` select the same startup modes as the CLI: custom boot is a direct-start path with the DMG boot-logo VRAM/map seed and no boot ROM asset requirement. When `SYSTEM -> START REAL` is selected but the configured Boot ROM file, Boot ROM directory, or active model-specific dump no longer exists, `gb-desktop` falls back to `skip-boot` for that session instead of aborting startup.

## Settings persistence

Persisted under the platform config directory by default, or under `GB_CYCLE_DESKTOP_SETTINGS_PATH` when that environment variable is set.

Persisted settings include:

- Launch hardware/session preferences: console model, startup mode, and compatibility mode.
- Boot-ROM preferences: selected firmware kind, search path, and verification mode.
- Battery-save preferences: enabled/disabled state, directory policy, and flush policy.
- Machine-state autoload slot.
- Rewind options.
- Frontend video: scale, fullscreen, vsync, integer-presentation, host-side presentation filter, LCD frame blending, desktop palette selection, stats-HUD visibility, and BG/WIN/OBJ presentation masks.
- Frontend audio: enabled state, volume, mute state, output sample rate, and buffer size.
- Audio channel selection and desktop `RECORD` state are intentionally **not** persisted, so new launches come up with the full mix selected and recording disabled unless the CLI recording flags explicitly requested otherwise.
- Keyboard joypad bindings and keyboard menu bindings.
- Frontend hotkeys.
- Fast Forward options.
- Gamepad bindings, gamepad host-action bindings, and gamepad menu bindings.
- Preferred SDL gamepad identity.
- Last opened directory and recent-ROM history.
