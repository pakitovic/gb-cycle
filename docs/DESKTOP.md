# gb-desktop

SDL3-based desktop frontend.

```bash
cargo run --release -p gb-desktop -- [path/to/rom.gb]
```

Can start without a ROM and wait in a launcher-style root menu until a ROM is selected.

On macOS, `CAM LIVE` needs the process to run from an app bundle that declares
`NSCameraUsageDescription`; otherwise the OS can leave SDL camera permission in
`pending` without listing the binary under Privacy & Security. Use the
development bundle launcher when testing Pocket Camera live input:

```bash
scripts/run-gb-desktop-macos-app -- [path/to/rom.gb]
```

The launcher builds `gb-desktop`, creates `target/macos/GB Cycle.app` with the
camera usage string from `crates/gb-desktop/macos/Info.plist`, ad-hoc signs it
when `codesign` is available, and launches it through LaunchServices so macOS
attributes camera permission to the `GB Cycle` bundle instead of the terminal or
editor process that ran the script. Because LaunchServices detaches standard
streams, this mode writes logs to `target/macos/gb-desktop.stdout.log` and
`target/macos/gb-desktop.stderr.log`. macOS should then prompt for **GB Cycle**
camera access, and the app appears under **System Settings -> Privacy &
Security -> Camera**. If LaunchServices reports a launch error such as `-10810`,
the script falls back to direct bundled execution; set
`GB_CYCLE_DESKTOP_LAUNCH_MODE=direct` only when you explicitly want that fallback
path for debugging.

For direct local `DMG-04` startup and reproducible profiling runs, the desktop
CLI also supports:

- `--link-rom <path>` to start immediately with a linked secondary cartridge
- `--exit-after-frames <n>` to exit automatically after presenting `n`
  emulated frames

Use `--release` for normal gameplay and timing-sensitive validation. Unoptimized `debug` builds are intended for development and may run well below real-time on commercial games.

## Core emulation

Reuses the same DMG-family startup model, startup mode, execution mode, boot-ROM search, and battery-save concepts as `gb-cli`.

Host audio playback consumes a typed post-HPF sample-capture boundary from `gb-core`, so the desktop frontend only performs final host-side `f32` normalization and SDL3 queueing instead of owning APU semantics.

## Audio investigation

- Use `--audio-record path/to/capture.wav` to export direct digital stereo APU output to `WAV` or `AIFC` without going through speakers, room acoustics, the macOS microphone path, or the frontend mute/volume controls.
- `--audio-record-rate <hz>` overrides the recording sink sample rate; the default is `96000` Hz so SameBoy-vs-`gb-cycle` commercial-ROM captures can be compared at the same host rate when desired.
- `--audio-record-stems <all|ch1,ch2,ch3,ch4>` writes isolated per-channel sidecar captures next to the main recording path (for example `capture.ch1.wav` and `capture.ch4.wav`) so commercial-ROM investigations can compare `CH1`, `CH2`, `CH3/WAVE`, and `CH4/NOISE` independently instead of only through the final mix.
- `AUDIO -> RECORD` in the desktop overlay starts/stops an automatic `WAV` capture at `96000` Hz under an `audios/` subdirectory next to the loaded ROM (for example `audios/zelda-0.wav`). That automatic recording uses the current desktop audio-channel selection instead of always forcing the full mix.
- `AUDIO -> CH1/CH2/CH3/CH4` are host-side diagnostic toggles. They do **not** rewrite `NR51` or any other APU register; they only change what the desktop frontend plays back or records.
- When all four channels are enabled, desktop playback and recording keep using the exact typed post-HPF `ApuHostSample` stream from `gb-core`. When a subset is selected, `gb-desktop` instead asks `gb-core` for the selected **pre-HPF** routed mix and then runs that mix through a dedicated host-side **T-cycle HPF state** before SDL playback or file encoding. That keeps the hardware model untouched while making solo/submix diagnosis practical under conditions that match SameBoy-style “mute other channels, then record the final mix” captures more closely than the old sample-rate DC-blocker path.
- Those per-channel stems start from the typed APU boundary **post-DAC / NR51 / NR50** and then pass through that same dedicated host-side **post-HPF** path before encoding. They are still investigative host exports, not a new claim that the hardware exposes an isolated solo-channel pin, but they now match the conditions of menu-driven final-mix recordings much more closely than the earlier pre-HPF + DC-block stems.
- Recording taps the same typed post-HPF `ApuHostSample` boundary that feeds SDL playback, but on an independent host sink, so it still works when normal desktop playback is muted or disabled for investigation.
- Set `GB_CYCLE_DESKTOP_AUDIO_LOG=1` to emit opt-in SDL audio telemetry to `stderr` with lightweight event logging only.
- Use `GB_CYCLE_DESKTOP_AUDIO_LOG=verbose` when you explicitly need one submit line per queued audio batch with queued-byte and queued-duration estimates.
- Set `GB_CYCLE_DESKTOP_AUDIO_DISABLE_AUTO_CLEAR=1` to disable the automatic oversized-queue recovery path entirely while investigating whether audible cuts come from host-side queue clears or from `gb-core`.
- Set `GB_CYCLE_DESKTOP_AUDIO_DISABLE_PACING_CORRECTION=1` to keep PR45 audio submit and queue behavior but remove the extra frame sleep derived from queued audio backlog, so host-side slowdown can be compared against the exact same `gb-desktop` build without switching branches.
- Both modes record stream pause/resume, queue clears, capture resets, mute, and volume changes so you can rule out host-side queue starvation or queue clears while reproducing a commercial-ROM audio issue in `gb-desktop`; they do not change APU timing or host audio policy.
- Steady-state frame pacing remains active even when renderer `vsync` is enabled, and applies a host-side correction from the current queued audio duration so normal gameplay converges toward roughly `100 ms` of queued audio instead of drifting upward when SDL presentation timing is loose.
- Automatic SDL queue clears remain a final emergency recovery path for catastrophic multi-second backlog, not the normal steady-state audio policy.

## Core trace capture

- Set `GB_CYCLE_DESKTOP_TRACE_PATH=/tmp/gb-desktop-trace.txt` to dump a rolling per-T-cycle CPU/APU/interrupt/joypad/bus trace when the frontend exits.
- Override the rolling window with `GB_CYCLE_DESKTOP_TRACE_T_CYCLES=<count>` when you need a narrower or wider final interaction window; the default is `8192` T-cycles.
- The rolling desktop trace now also appends the cartridge trace summary from `gb-core`; for HuC1 investigations that includes the current mapper state plus `io_mode`, raw/effective ROM bank, raw/effective RAM bank, and the IR emitter / sensor bits.
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
- This capture is intended for interactive commercial-ROM investigation when reproducing the issue directly in `gb-desktop` is easier than scripting the same input path in `gb-test-runner`.
- When trace capture is disabled, the desktop loop now skips the trace-record path entirely instead of calling into the ring-buffer helper on every `T-cycle`.

## Display and performance

- Opens a desktop window and renders the live `160x144` framebuffer.
- Host-side presentation filtering now defaults to `OFF`, so the SDL texture is sampled with nearest-neighbor scaling unless `VIDEO -> FILTER` is enabled for linear smoothing.
- `VIDEO -> BACKGROUND`, `VIDEO -> WINDOW`, and `VIDEO -> OBJECTS` are debug presentation masks that do not touch core timing or `LCDC` state. Disabling `OBJECTS` reveals the stored BG/WIN plane underneath in both the live window and screenshots; if `BACKGROUND` and/or `WINDOW` are also masked away, the uncovered area now falls back to the per-pixel DMG backdrop shade (palette entry `0` under the historical `BGP` value) instead of a fixed solid fill, so OBJ-only captures track SameBoy's changing diagnostic backdrop more closely. Disabling `BACKGROUND` or `WINDOW` still masks that plane by source rather than recomputing a fresh behind-window raster.
- `VIDEO -> SCREENSHOT` saves a native-size PNG next to the running ROM inside a `screenshots/` subdirectory using an `8-bit RGB` layout similar to SameBoy’s raw screenshots, without baking in host-side scaling, filtering, HUD, or menu overlays.
- Window title reports live FPS, average frame time, relative emulation speed, and a frontend-side breakdown of average emulation, render, pacing, and audio-queue timing.
- Compact in-window performance HUD with those same frontend metrics, toggleable from `VIDEO` or through a dedicated remappable hotkey.
- Set `GB_CYCLE_DESKTOP_EMU_PROFILE=1` or `GB_CYCLE_DESKTOP_EMU_PROFILE=summary` to emit opt-in sampled `EMU` breakdowns to `stderr`.
- The default mode replays one cloned start-of-frame state every `15` presented frames on a background worker, then reports sampled averages for the real measured frame time plus normalized `gb-core` estimates for `CPU`, `PPU`, and the remaining core buckets split into external-event ingress, timer, APU, DMA, serial, and interrupt handling.
- The sampled `PPU` bucket is further split into `mode0_1`, `mode2`, `mode3_startup`, background fetch, window fetch/restart, BG push/fill, OBJ fetch, pixel transfer, and a `ppu_other` remainder so menu and HUD slowdowns can be narrowed to a specific raster phase without instrumenting the main thread.
- Coarse frontend work that still lives inside the measured emulation window remains reported from the real frame (`SDL` event polling, audio submit, save flush), and the summary also emits sampled `frame_tcycles`, `frame_start_ly`, `frame_start_dot`, `frame_end_ly`, `frame_end_dot`, `frame_crossings`, `scanline_transitions`, `scanlines_over_456`, `max_scanline_tcycles`, `max_scanline_ly`, `max_mode0_start_dot`, `max_mode0_start_dot_ly`, `ly153_to0`, `ly153_to0_startup`, `ly153_to0_blank`, `ly0_self_wraps`, `ly0_self_wrap_startup`, `ly0_self_wrap_blank`, `ly0_to1`, `ly0_tcycles`, `ly0_max_mode0_start_dot`, `ly0_stall_tcycles`, `ly0_stall_hb_tcycles`, `ly0_stall_oam_tcycles`, `ly0_stall_draw_tcycles`, `ly0_stall_startup_tcycles`, `ly0_stall_blank_tcycles`, `ly0_stall_runs`, `ly0_max_stall_tcycles`, `ly0_max_stall_dot`, `ly0_max_stall_mode_dot`, `cpu_stop_tcycles`, `cpu_zstop_tcycles`, `ly0_stop_tcycles`, `ly0_zstop_tcycles`, `ly0_stall_stop_tcycles`, `ly0_stall_zstop_tcycles`, `lcdoff_tcycles`, `lcdoff_transitions`, `lcdon_transitions`, `ly0_lcdoff_tcycles`, `ly0_stall_lcdoff_tcycles`, `submit_samples`, `submit_tcycles`, `submit_queue_before_ms`, `submit_enqueued_ms`, `submit_queue_after_ms`, `audio_queue_before_ms`, and `audio_queue_after_ms` plus host-side `present_ms`, `pac_ms`, `sleep_target_ms`, `audio_corr_ms`, `late_ms`, and `oversleep_ms` so compositor or pacing jitter can be separated from core cost and correlated with backlog-driven audio correction, including direct `LY=0` stall detection at the frame boundary, whether it overlaps `STOP`/`ZombieStopped`, and whether the PPU actually enters LCD-off state inside the bad frame.
- Summary lines also tag the active session shape as `session=single` or
  `session=linked-dmg04-2p` so the single-console and linked runs can be
  compared mechanically from the same profiler output stream.
- The detailed frame-boundary, scanline, and `LY=0` stall counters are only
  collected while `GB_CYCLE_DESKTOP_EMU_PROFILE` is enabled, so normal desktop
  gameplay does not pay that extra per-`T-cycle` frontend bookkeeping cost.
- Linked-session profile replays now also use the linked observer stepping
  path, so sampled `CPU`, `PPU`, and `core_other` buckets remain populated for
  local `DMG-04` runs instead of collapsing to zero during the background
  replay.
- Normal gameplay also skips repeated rumble synchronization work for
  cartridges without rumble support unless the frontend still has an applied
  host rumble effect to clear.
- Override the sampling stride with `GB_CYCLE_DESKTOP_EMU_PROFILE=summary:<frames>` when you need denser or lighter sampling during an investigation.
- That profiler is investigative timing only: it does not alter emulation semantics, it is disabled by default, and it is designed to minimize main-thread intrusion while still separating likely core cost from host overhead when a commercial ROM path drops below full speed.

For the current Phase `7.6.a` baseline, use the same release build and SDL
dummy drivers for both runs so the profiler compares desktop host overhead and
linked-session cost under the same conditions:

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

### Rebinding

All rebinding takes immediate runtime effect:

- `INPUT -> KEYBOARD` — in-window keyboard joypad rebinding.
- `INPUT -> KB MENU` — dedicated host-side keyboard menu rebinding.
- `INPUT -> HOTKEYS` — frontend hotkey rebinding.
- Keyboard rebinding uses SDL3 physical scancodes when available so saved bindings stay stable across host layouts. Supported keyboard keys include the existing arrows, `Backspace`, `Enter`, `Space`, `R`, `X`, `Z`, function hotkeys, plus `Tab`, left/right `Shift`, left/right `Control`, left/right `Alt` (`Option` on macOS), and left/right GUI (`Command` on macOS, Windows/Super on Windows/Linux). `Fn` remains host/firmware-owned and is not treated as a reliable bindable key.
- `INPUT -> GAMEPAD` — SDL gamepad rebinding.
- `INPUT -> PAD MENU` — dedicated SDL gamepad menu rebinding.
- `INPUT -> RUMBLE` — host rumble mode for the active SDL gamepad with `OFF`, `HIGH`, and `LOW` host-intensity options. This option is only enabled when the loaded cartridge exposes rumble support and the active gamepad reports SDL rumble capability; otherwise it remains visible but disabled.

## Overlay and menus

Pause/menu overlay with native SDL3 `Open ROM` filtered to common Game Boy ROM extensions, plus frontend-owned submenus:

- `Escape` and the active gamepad `Guide` button open the overlay when it is closed; when it is already open they both act as the same back/cancel control.
- In launcher mode without a loaded ROM, that shared back/cancel behavior does not dismiss the root overlay.
- While a native file dialog is pending from the overlay, the triggering entry stays selected but disabled until the dialog resolves.
- Root overlay also exposes `QUIT` directly at the first menu level.
- Root-level back/cancel (`Escape` / `Guide`) clears an explicit manual `SPACE` pause before closing the overlay, and loading a new primary ROM from `OPEN ROM` / `OPEN RECENT` also leaves the frontend unpaused so screenshot/debug workflows do not strand the session in a hidden paused state.
- When the loaded session includes a `Pocket Camera` cartridge, the root overlay also exposes:
  - `CAM LIVE ON` / `CAM LIVE OFF` — opens or stops the first SDL3 camera device with SDL's native stream selection, converts each available frame to grayscale, mirrors it horizontally for self-facing Pocket Camera orientation, and pushes it through the same core API used by `CAM IMAGE`
  - `CAM IMAGE` — native PNG picker that decodes the selected image in the frontend and pushes it into the core as a grayscale host frame
  - `CAM RESET` — stops live capture if active, clears the current session image, and restores the core's deterministic placeholder frame
  These entries appear before the general ROM/system menus so Camera ROM sessions expose live capture state first.
- Pocket Camera still-image selection and live-camera state are session-scoped only. A chosen still image is reapplied across ROM reloads / resets while the desktop app stays open, but neither still-image path nor live-camera state is persisted into desktop settings.
- Camera permission, device selection, native frame acquisition, RGB conversion, horizontal live-frame mirroring, and warm-up frame dropping are frontend-owned. `gb-core` only receives grayscale host frames and performs the deterministic `128x112` normalization.
- If SDL opens a camera but no frames arrive, the desktop log reports whether SDL still considers camera permission `pending`, `approved`, or `denied`; this keeps OS permission stalls distinguishable from frame acquisition stalls.
- **`VIDEO`** — stats HUD visibility, host-side presentation filter, fullscreen, vsync, window scale, integer presentation, screenshot capture, and BG/WIN/OBJ presentation masks.
- **`AUDIO`** — toggle mute, cycle host volume, host-mask `CH1..CH4`, and start/stop automatic `WAV` captures under `audios/`.
- **`INPUT`** — keyboard, gamepad, hotkey, and menu rebinding (see above).
- **`SYSTEM`** — system-level options such as console model, startup mode, execution mode, the `BOOT ROM` submenu, the `SAVE` submenu, and reset.
- **`SYSTEM -> BOOT ROM`** — boot-ROM-specific options: `BOOT AUTO`,
  `BOOT FILE`, `BOOT DIR`, and `VERIFY`. `MODEL`, `START`, and `SAVE` remain
  at the system/save level so hardware model selection, startup policy, and
  cartridge persistence are not mixed with boot-ROM asset configuration.
- **`SYSTEM -> SAVE`** — save-specific options: `EXPORT SAVE`, `IMPORT SAVE`,
  `SAVE BATTERY`, `SAVES ON/OFF`, `SAVE POLICY`, `DIR AUTO`, and `SAVE DIR`.
  This keeps cartridge persistence controls in one submenu instead of mixing
  them into the root or top-level system option list.
- **`OPEN RECENT`** — recent-ROM history for the last `12` ROMs, available from the root overlay whenever recent ROMs exist; entries can relaunch directly, the submenu exposes `CLEAR LIST`, and the selected entry scrolls after a short dwell when the sanitized title is wider than the overlay text area.
- **`DEFAULTS`** — reset actions inside `VIDEO`, `AUDIO`, and `INPUT` to restore host-side settings and bindings without touching CLI config.

## Battery saves

- Default policy: debounced auto-flush — once cartridge persistence changes, the frontend writes a safe replacement save after roughly `2s`, and forces a flush on ROM changes and shutdown.
- For RTC-backed `MBC3` cartridges, the desktop loop also injects host wall-clock elapsed seconds into the live session, so clock-based games keep advancing while the ROM remains open instead of only catching up on the next save reload.
- `SYSTEM -> SAVE -> EXPORT SAVE` writes the current primary/P1 cartridge
  persistence as a SameBoy/mGBA-compatible `.sav`. The native save dialog
  defaults under `saves/export` next to the active ROM or configured save root.
- `SYSTEM -> SAVE -> IMPORT SAVE` reads a SameBoy/mGBA-compatible `.sav`,
  validates it against the current primary/P1 ROM, writes the matching internal
  `.gbsav`, and then asks the user to reload or reset the game. V1 does not
  hot-swap the live cartridge session; after a successful import the active
  primary save session is disabled until reload so the running game cannot
  overwrite the imported `.gbsav`.
- The external `.sav` compatibility boundary mirrors the CLI converter: linear
  cartridge RAM is raw bytes, `MBC3` RTC saves use the shared `48`-byte suffix,
  and `MBC2` import accepts SameBoy and mGBA layouts while export defaults to
  mGBA packed bytes.
- The `SAVE BATTERY` menu action is only exposed inside `SYSTEM -> SAVE` when
  the desktop save policy is explicitly set to `manual`.

## Error handling

User-facing desktop failures such as ROM open/load errors surface through native SDL3 message boxes instead of only writing to `stderr`; technical diagnostics remain in terminal output.
When `SYSTEM -> START REAL` is selected but the configured Boot ROM file, Boot ROM directory, or active model-specific dump no longer exists, `gb-desktop` falls back to `skip-boot` for that session instead of aborting startup.

## Settings persistence

Persisted under the platform config directory by default, or under `GB_CYCLE_DESKTOP_SETTINGS_PATH` when that environment variable is set.

Persisted settings include:

- Frontend video: scale, vsync, integer-presentation, host-side presentation filter, stats-HUD visibility.
- Frontend audio: volume, mute state.
- Audio channel selection and desktop `RECORD` state are intentionally **not** persisted, so new launches come up with the full mix selected and recording disabled unless the CLI recording flags explicitly requested otherwise.
- Keyboard joypad bindings and keyboard menu bindings.
- Frontend hotkeys.
- Gamepad bindings and gamepad menu bindings.
- Preferred SDL gamepad identity.
- Last opened directory and recent-ROM history.
