# gb-desktop

SDL3-based desktop frontend.

```bash
cargo run --release -p gb-desktop -- [path/to/rom.gb]
```

Can start without a ROM and wait in a launcher-style root menu until a ROM is selected.

Use `--release` for normal gameplay and timing-sensitive validation. Unoptimized `debug` builds are intended for development and may run well below real-time on commercial games.

## Core emulation

Reuses the same DMG-family startup model, startup mode, execution mode, boot-ROM search, and battery-save concepts as `gb-cli`.

Host audio playback consumes a typed post-HPF sample-capture boundary from `gb-core`, so the desktop frontend only performs final host-side `f32` normalization and SDL3 queueing instead of owning APU semantics.

## Audio investigation

- Set `GB_CYCLE_DESKTOP_AUDIO_LOG=1` to emit opt-in SDL audio telemetry to `stderr` with lightweight event logging only.
- Use `GB_CYCLE_DESKTOP_AUDIO_LOG=verbose` when you explicitly need one submit line per queued audio batch with queued-byte and queued-duration estimates.
- Set `GB_CYCLE_DESKTOP_AUDIO_DISABLE_AUTO_CLEAR=1` to disable the automatic oversized-queue recovery path entirely while investigating whether audible cuts come from host-side queue clears or from `gb-core`.
- Both modes record stream pause/resume, queue clears, capture resets, mute, and volume changes so you can rule out host-side queue starvation or queue clears while reproducing a commercial-ROM audio issue in `gb-desktop`; they do not change APU timing or host audio policy.
- Steady-state frame pacing remains active even when renderer `vsync` is enabled, and applies a host-side correction from the current queued audio duration so normal gameplay converges toward roughly `100 ms` of queued audio instead of drifting upward when SDL presentation timing is loose.
- Automatic SDL queue clears remain a final emergency recovery path for catastrophic multi-second backlog, not the normal steady-state audio policy.

## Core trace capture

- Set `GB_CYCLE_DESKTOP_TRACE_PATH=/tmp/gb-desktop-trace.txt` to dump a rolling per-T-cycle CPU/APU/interrupt/joypad/bus trace when the frontend exits.
- Override the rolling window with `GB_CYCLE_DESKTOP_TRACE_T_CYCLES=<count>` when you need a narrower or wider final interaction window; the default is `8192` T-cycles.
- Cycles that include an APU MMIO write in `FF10..FF26` append `apu.last_write=... before(...) after(...)` so you can correlate audible glitches with the exact register transition that just occurred.
- This capture is intended for interactive commercial-ROM investigation when reproducing the issue directly in `gb-desktop` is easier than scripting the same input path in `gb-test-runner`.

## Display and performance

- Opens a desktop window and renders the live `160x144` framebuffer.
- Window title reports live FPS, average frame time, relative emulation speed, and a frontend-side breakdown of average emulation, render, pacing, and audio-queue timing.
- Compact in-window performance HUD with those same frontend metrics, toggleable from `VIDEO` or through a dedicated remappable hotkey.

## Input

- Maps keyboard and SDL3 gamepad input to the joypad path.
- Basic gamepad hotplug plus preferred-device selection and remappable gamepad bindings.
- Shows the current active gamepad in the `GAMEPAD` submenu; can pin or clear the preferred device from that UI.
- Can move gamepad focus to the last used controller whenever no preferred device is currently locked.

### Rebinding

All rebinding takes immediate runtime effect:

- `INPUT -> KEYBOARD` — in-window keyboard joypad rebinding.
- `INPUT -> KB MENU` — dedicated host-side keyboard menu rebinding.
- `INPUT -> HOTKEYS` — frontend hotkey rebinding.
- `INPUT -> GAMEPAD` — SDL gamepad rebinding.
- `INPUT -> PAD MENU` — dedicated SDL gamepad menu rebinding.
- `INPUT -> RUMBLE` — host rumble mode for the active SDL gamepad with `OFF`, `HIGH`, and `LOW` host-intensity options. This option is only enabled when the loaded cartridge exposes rumble support and the active gamepad reports SDL rumble capability; otherwise it remains visible but disabled.

## Overlay and menus

Pause/menu overlay with native SDL3 `Open ROM` filtered to common Game Boy ROM extensions, plus frontend-owned submenus:

- **`VIDEO`** — fullscreen, vsync, window scale, integer presentation, stats HUD visibility.
- **`AUDIO`** — toggle mute, cycle host volume.
- **`INPUT`** — keyboard, gamepad, hotkey, and menu rebinding (see above).
- **`SYSTEM`** — system-level options.
- **`OPEN RECENT`** — recent-ROM history, available from the root overlay whenever recent ROMs exist; entries can relaunch directly.
- **`DEFAULTS`** — reset actions inside `VIDEO`, `AUDIO`, and `INPUT` to restore host-side settings and bindings without touching CLI config.

## Battery saves

- Default policy: debounced auto-flush — once cartridge persistence changes, the frontend writes a safe replacement save after roughly `2s`, and forces a flush on ROM changes and shutdown.
- For RTC-backed `MBC3` cartridges, the desktop loop also injects host wall-clock elapsed seconds into the live session, so clock-based games keep advancing while the ROM remains open instead of only catching up on the next save reload.
- The `SAVE BATTERY` menu action is only exposed when the desktop save policy is explicitly set to `manual`.

## Error handling

User-facing desktop failures such as ROM open/load errors surface through native SDL3 message boxes instead of only writing to `stderr`; technical diagnostics remain in terminal output.
When `SYSTEM -> START REAL` is selected but the configured Boot ROM file, Boot ROM directory, or active model-specific dump no longer exists, `gb-desktop` falls back to `skip-boot` for that session instead of aborting startup.

## Settings persistence

Persisted under the platform config directory by default, or under `GB_CYCLE_DESKTOP_SETTINGS_PATH` when that environment variable is set.

Persisted settings include:

- Frontend video: scale, vsync, integer-presentation, stats-HUD visibility.
- Frontend audio: volume, mute state.
- Keyboard joypad bindings and keyboard menu bindings.
- Frontend hotkeys.
- Gamepad bindings and gamepad menu bindings.
- Preferred SDL gamepad identity.
- Last opened directory and recent-ROM history.
