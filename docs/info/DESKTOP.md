# gb-desktop

`gb-desktop` is the SDL3 desktop frontend. It owns host UX, windowing, menus, input devices, audio delivery, host pacing, settings, screenshots, local packaging, and investigation-only traces; it must reuse `gb-core` for hardware semantics and must not redefine model, startup, timing, compatibility, or persistence behavior. For the matching headless contract see [`CLI.md`](CLI.md), for model axes see [`MODEL-AXES.md`](MODEL-AXES.md), for boot/startup semantics see [`../hardware/BOOT-ROM.md`](../hardware/BOOT-ROM.md), for timing policy see [`TIMING-AND-ACCURACY.md`](TIMING-AND-ACCURACY.md), and for automated ROM-suite policy see [`ROM-SUITES.md`](ROM-SUITES.md) plus [`../TESTING.md`](../TESTING.md).

## Running

```bash
cargo run --release -p gb-desktop -- [path/to/rom.gb]
cargo run --profile release-max -p gb-desktop -- [path/to/rom.gb]
```

Use `--release` for normal gameplay and timing-sensitive validation. `debug` builds are for development and may run below real time on commercial games. `release-max` enables the workspace's slower but more optimized desktop profile with fat LTO, one codegen unit, stripped symbols, and abort-on-panic behavior.

If no ROM is provided, the app opens the in-window launcher root menu and waits for `OPEN ROM`, `OPEN RECENT`, or configuration changes before a cartridge session exists.

On macOS, `CAM LIVE` needs an app bundle declaring `NSCameraUsageDescription`; direct terminal launches may leave SDL camera permission in `pending` without listing the binary under Privacy & Security. Use `scripts/run-gb-desktop-macos-app.sh -- [rom]` for Pocket Camera live-input testing. The helper builds `gb-desktop`, creates `target/macos/GB Cycle.app` from `crates/gb-desktop/macos/Info.plist`, ad-hoc signs when possible, launches through LaunchServices, and writes detached stdout/stderr logs to `target/macos/gb-desktop.stdout.log` and `target/macos/gb-desktop.stderr.log`. Set `GB_CYCLE_DESKTOP_LAUNCH_MODE=direct` only when deliberately bypassing LaunchServices for debugging.

## CLI surface

Run `gb-desktop --help` for the exhaustive flag list. The stable desktop-specific groups are:

| Area | Options |
| --- | --- |
| Hardware policy | `--model <DMG|MGB|LGB|CGB|AGB|SGB|SGB2>`, `--revision <dmg-cpu-0|dmg-cpu-c|cpu-mgb|cpu-cgb-0|cpu-cgb-c|cpu-cgb-d|cpu-cgb-e|cpu-agb-0|cpu-agb-a>`, `--sgb-standard <ntsc|pal>`, `--startup <skip-boot|custom-boot|real-boot>`, `--mode <strict|permissive|experimental>`, `--boot-rom-dir <dir>`, `--boot-rom-verify <off|warn|strict>` |
| Automation | `--test-runner`, `--benchmark <case.toml>`, `--exit-after-frames <n>` |
| Local links | `--link-rom <path>` for a local 2-player `DMG-04` session only |
| Cartridge saves | `--save-dir <dir>`, `--save-key <key>`, `--save-policy <manual|on-close|on-write|debounced>`, `--no-saves` |
| Presentation | `--scale <n>`, `--fullscreen`, `--no-vsync`, `--palette grey`, `--frame-blend <off|on>` |
| Audio | `--mute`, `--audio-record <path.wav|path.aifc>`, `--audio-record-rate <hz>`, `--audio-record-stems <all|ch1,ch2,ch3,ch4>` |
| Input | `--no-gamepad`, gamepad direction/face-layout/binding options, and preferred SDL gamepad name/path options |

`--test-runner` applies host-light automation defaults after parsing: permissive compatibility mode, the ordinary DMG-only grey-palette rule, `BORDER OFF`, disabled saves, disabled rewind, window scale `1`, no fullscreen, no vsync, muted audio, disabled gamepad, and hidden performance HUD unless a specific host override was passed explicitly. It still uses the normal T-cycle core, host RTC sync, and frame pacer; it only trims desktop host work so screenshot and benchmark runners are reproducible.

`--benchmark <case.toml>` loads one portable benchmark case through `gb-benchmark`. The TOML owns the ROM path, model, startup, mode, artifact toggles, and one or more `[[run]]` entries with deterministic `[[run.input]]` pulses; each run starts a fresh desktop session and writes `gb-desktop/<artifact-id>.png` plus `gb-desktop/<artifact-id>-stats.toml` when enabled. Relative `rom` paths resolve against the TOML directory, `id` / `run.id` must be filename-safe ASCII, and the old top-level `duration_seconds` plus `[[stimulus]]` format is rejected.

`cargo rom-bench` is the local benchmark helper. By default it builds/runs `gb-desktop` cases under `test/bench/gb-desktop/` and writes `test/bench/index.html`; add `--gb-cli` to also run `gb-cli` and include CLI columns. Useful maintenance modes are `--sample`, `--test <case.toml>`, `--rom-dir <dir>`, `--normalize-case`, and `--generate-cases [--template <case.toml>]`.

Local `DMG-07` 4-Player Adapter sessions and CGB IR/accessory sessions are overlay-only. Desktop intentionally has no CLI shortcut for 3P/4P startup, `IR: SAME GAME`, `IR: SELECT GAME`, `IR: PIKACHU 2`, or `IR: MYSTERY GIFT`.

## Model, startup, and mode

The desktop model contract mirrors [`CLI.md`](CLI.md): public model names are `DMG`, `MGB`, `LGB`, `CGB`, `AGB`, `SGB`, and `SGB2`; old product names and lowercase legacy aliases are rejected. `AGB` is displayed as `MODEL GB ADVANCE` and maps to the active AGB GB/C compatibility profile; there are intentionally no separate `GBA SP` or `GB PLAYER` UI models because the current core-visible GBA-enhanced behavior is the same. `SGB` and `SGB2` are frontend machine profiles that wrap the shared DMG-compatible GB core in an SGB host profile, not CGB mode and not a forked GB core.

`--revision` and `CONFIG -> SYSTEM -> REV` select the active revision for the chosen model profile. `DMG` cycles through `DMG-CPU 0` and `DMG-CPU C`; `CGB` cycles through `CGB-CPU 0`, `CPU CGB C`, `CPU CGB D`, and `CPU CGB E`; `MGB` and `LGB` have one active revision each; `AGB` cycles through `CPU AGB 0` and `CPU AGB A` with `CPU AGB A` as the default; `SGB` and `SGB2` use their SGB-profile-backed `DMG-CPU C` GB core and do not expose `DMG-CPU 0`, because the SGB startup state is selected by `SgbHostProfile` rather than by DMG0 handheld boot rules.

`--sgb-standard <ntsc|pal>` and `CONFIG -> SYSTEM -> VIDEO NTSC/PAL` apply only to original `SGB`. `SGB2` is fixed to its corrected NTSC SGB2 host profile and shows the item disabled rather than pretending a PAL variant exists.

`CONFIG -> SYSTEM -> BOOT ROM` owns firmware search path and verification policy only. `RealBoot` derives the concrete firmware image from the effective model/revision/host profile, for example `cgbE_boot.bin`, `cgb_agb0_boot.bin`, `cgb_agb_boot.bin`, `sgb_boot.bin`, or `sgb2_boot.bin`; skip/custom boot still use the selected revision/profile for direct-start state but do not read boot-ROM bytes. `AGB` uses `cgb_agb0_boot.bin` for `CPU AGB 0` real boot and `cgb_agb_boot.bin` for `CPU AGB A` real boot; it does not require or load `gba_bios.bin`.

`--mode` and `CONFIG -> SYSTEM -> MODE` apply the complete `Strict`, `Permissive`, or `Experimental` compatibility preset. When a ROM is already loaded, cycling mode rebuilds the session under the next compatible preset; if the next preset would reject the cartridge metadata, desktop logs the skipped-mode reason to `stderr` and tries the following preset instead of interrupting the menu flow with a modal load error.

## Presentation, pacing, and performance

`CONFIG -> SYSTEM -> BORDER` is available for `GAME BOY`, `GB POCKET`, `GB LIGHT`, `GB COLOR`, `GB ADVANCE`, `SUPER GB`, and `SUPER GB2` as `BORDER AUTO` / `BORDER OFF`. `AUTO` renders the `256x224` SGB host frame for loaded `SGB`/`SGB2` sessions, renders a `256x224` borrowed SGB border for handheld sessions only when the ROM header declares SGB support and an initial `PCT_TRN` border can be extracted, and otherwise falls back to the native `160x144` LCD. `OFF` always hides the border: SGB/SGB2 sessions render the SGB-colored `160x144` LCD RGB555 output, while handheld sessions render the active model's native `160x144` output. Local linked layouts render native-or-bordered panels side by side for `DMG-04`, CGB IR, and 2-player `DMG-07`, or a `2x2` grid for 3-/4-player `DMG-07`.

`--palette grey` and `CONFIG -> VIDEO -> PALETTE` are desktop presentation controls for DMG-family output only. The menu provides model-aware defaults for `GAME BOY`, `GB POCKET`, and `GB LIGHT`; CGB, AGB, and SGB-family sessions render RGB555 output directly. These palette choices affect the live window and desktop screenshots, not `gb-core`, CLI artifacts, ROM-test oracles, or rank-normalized test output.

`CONFIG -> VIDEO` owns host filtering, LCD frame blending, fullscreen/vsync/window scale/integer presentation, screenshot capture, stats HUD, and BG/WIN/OBJ debug masks. `CONFIG -> VIDEO -> SCREENSHOT` writes a native-size PNG under `screenshots/` next to the running ROM without host scaling, filtering, frame blending, HUD, or menu overlays; SGB/SGB2 screenshots and handheld borrowed-border screenshots follow the active `BORDER AUTO/OFF` presentation.

Desktop frame pacing and the performance HUD speed percentage use the effective GB master clock for the selected profile. Handheld and `SUPER GB2` profiles target about `59.73` GB frames/s, original `SUPER GB` NTSC targets about `61.17` GB frames/s, and original `SUPER GB` PAL targets about `60.61` GB frames/s. Host vsync can still clamp observed window FPS, but `% speed` is computed against the selected hardware profile cadence.

`CONFIG -> SYSTEM -> F-FORWARD` and the default `Right Shift` hold hotkey are desktop-only host acceleration controls. Fast Forward skips host pacing sleeps, temporarily suppresses renderer vsync, skips SDL playback-audio submission, and clears host audio queue backlog while held; it does not change `gb-core` timing or write into save states. `CONFIG -> SYSTEM -> REWIND` and the default `Left Shift` hold hotkey restore older in-memory `.gbstate` snapshots through the normal core restore path; linked/local multi-machine sessions disable rewind by design.

`GB_CYCLE_DESKTOP_EMU_PROFILE=summary[:frames]`, `summary-lite[:frames]`, or `summary-overhead[:frames]` emits opt-in sampled timing breakdowns to `stderr`. This profiler is investigative timing only: it is disabled by default, avoids normal-gameplay per-T-cycle observer cost, and should be used to separate likely core cost from host overhead before changing emulator semantics.

## Audio investigation

Desktop playback consumes the typed post-HPF `ApuHostSample` boundary from `gb-core`; the frontend only performs final host-side normalization, SDL3 queueing, and optional recording. In CGB double-speed, desktop captures that host boundary only on the undoubled APU/LCD-domain tick, so a double-speed video frame enqueues roughly one frame of host audio rather than two.

`--audio-record <path.wav|path.aifc>` writes direct digital stereo output before frontend mute/volume controls. `--audio-record-rate <hz>` defaults to `96000`. `--audio-record-stems <all|ch1,ch2,ch3,ch4>` writes isolated sidecars next to the main recording path for channel investigation.

`CONFIG -> AUDIO -> RECORD` starts/stops automatic `WAV` capture under an `audios/` directory next to the loaded ROM and uses the current desktop channel selection. `CONFIG -> AUDIO -> CH1/CH2/CH3/CH4` are host diagnostic masks: they do not rewrite `NR51`, `NR50`, or any APU register. Full-mix playback/recording uses the typed post-HPF stream; selected submixes ask `gb-core` for the chosen pre-HPF routed mix and then run a dedicated host-side T-cycle HPF before playback/encoding.

Useful audio environment variables are `GB_CYCLE_DESKTOP_AUDIO_LOG=1` for event telemetry, `GB_CYCLE_DESKTOP_AUDIO_LOG=verbose` for per-submit queue estimates, `GB_CYCLE_DESKTOP_AUDIO_DISABLE_AUTO_CLEAR=1` to disable oversized SDL queue recovery, and `GB_CYCLE_DESKTOP_AUDIO_DISABLE_PACING_CORRECTION=1` to keep audio submission unchanged while removing backlog-derived extra frame sleep. These switches help isolate host delivery issues and do not alter APU timing.

## Trace captures

Desktop trace captures are for interactive commercial-ROM investigation when reproducing a path in the frontend is easier than scripting it in `gb-test-runner`. Disabled captures skip their per-T-cycle hook entirely.

| Environment | Purpose |
| --- | --- |
| `GB_CYCLE_DESKTOP_TRACE_PATH` / `GB_CYCLE_DESKTOP_TRACE_T_CYCLES` | Rolling per-T-cycle CPU/APU/interrupt/joypad/bus trace, default window `8192` T-cycles. |
| `GB_CYCLE_DESKTOP_WATCH_TRACE_PATH` / `GB_CYCLE_DESKTOP_WATCH_TRACE_ADDRESSES` / `GB_CYCLE_DESKTOP_WATCH_TRACE_EVENTS` | Condensed trace for watched CPU bus/address activity. |
| `GB_CYCLE_DESKTOP_PC_WATCH_TRACE_PATH` / `GB_CYCLE_DESKTOP_PC_WATCH_TRACE_RANGES` / `GB_CYCLE_DESKTOP_PC_WATCH_TRACE_EVENTS` | Condensed trace for inclusive PC ranges. |
| `GB_CYCLE_DESKTOP_EDGE_TRACE_PATH` / `GB_CYCLE_DESKTOP_EDGE_TRACE_ADDRESSES` / `GB_CYCLE_DESKTOP_EDGE_TRACE_PC_RANGES` / `GB_CYCLE_DESKTOP_EDGE_TRACE_EVENTS` | Edge-style trace that records watched value changes and PC-range entry context. |
| `GB_CYCLE_DESKTOP_CGB_IR_TRACE_PATH` / `GB_CYCLE_DESKTOP_CGB_IR_TRACE_WATCH_ADDRESSES` / `GB_CYCLE_DESKTOP_CGB_IR_TRACE_TRIGGER_ADDRESSES` / `GB_CYCLE_DESKTOP_CGB_IR_TRACE_EVENTS` | CGB IR RP/status transition trace with optional watched CPU addresses. |
| `GB_CYCLE_DESKTOP_CGB_IR_OPTICAL_DELAY_T_CYCLES` | Investigation-only override for the provisional CGB IR optical edge delay. |
| `GB_CYCLE_DESKTOP_CH4_NR43_TRACE_PATH` | Condensed CH4/NR43 live-write trace with noise-channel debug state. |
| `GB_CYCLE_DESKTOP_CH4_STARTUP_TRACE_PATH` | CH4 startup events, including relevant `NR52`, `NR42`, `NR43`, and `NR44` writes plus delayed-start firing. |
| `GB_CYCLE_DESKTOP_CPU_WINDOW_TRACE_PATH` | Fixed legacy CPU/PPU window trace for a known execution window; not a general filter. |

## Input and overlay

Keyboard and SDL3 gamepad input enter the joypad path through frontend-owned bindings. Desktop clamps impossible stock-handheld direction pairs (`Left+Right`, `Up+Down`) to neutral before they reach the core, even when mixed host sources produce them.

Default P1 keyboard bindings are arrow keys, `Left Option` for `B`, `Left Command` for `A`, `Backspace` for `SELECT`, and `Enter` for `START`. Default hotkeys are `Space` pause, `F1` save state, `F2` load state, `1`/`2`/`3`/`4` state-slot selection, `Left Shift` rewind, `Right Shift` Fast Forward, `F9` manual battery save, `F10` stats HUD, `F11` fullscreen, and `F12` reset.

Local multi-console sessions use fixed keyboard profiles for extra players: `P2` uses `WASD` plus `Z/X` and `Q/E`, `P3` uses `TFGH` plus `V/B` and `R/Y`, and `P4` uses `IJKL` plus `M/,` and `U/O`. SGB/SGB2 `MLT_REQ` games reuse those P2/P3/P4 profiles as SGB host controller slots; gamepad input remains assigned to P1 only.

`CONFIG -> INPUT` owns keyboard, keyboard-menu, hotkey, gamepad, gamepad-menu, host-action, preferred-device, rumble, and MBC7 tilt/accelerometer rebinding. Rebinding takes effect immediately and is persisted with desktop settings. `CONFIG -> INPUT -> GYRO` is named for the host source but feeds MBC7 accelerometer/tilt input because MBC7 models ADXL202E acceleration rather than angular velocity.

`Escape` and the active gamepad `Guide` button open the overlay when closed and act as back/cancel when open. Launcher mode keeps the root overlay open until a ROM is selected. Native file dialogs disable their triggering item until the dialog resolves, and user-facing failures surface through SDL message boxes while technical details stay on `stderr`. `OPEN ROM` starts in the loaded ROM directory, then the persisted last-open ROM directory, then the current working directory, and filters cartridge files to `.gb` and `.gbc` where SDL's native dialog backend supports file filters.

The root overlay exposes ROM loading/recent history, single-machine `.gbstate` save/load/autoload slots after a ROM is loaded, Pocket Camera controls when the loaded cartridge supports them, `EXT. PORT`, CGB `IR`, `CONFIG`, and `QUIT`. Root-level cancel clears explicit manual pause before returning to gameplay, and loading a new ROM leaves the frontend unpaused.

`EXT. PORT` covers `NONE`, `PRINTER`, `GAME LINK`, and `4P ADAPTER`. `GAME LINK` can clone the current ROM into a local same-game `DMG-04` session or ask for a second ROM; `4P ADAPTER` rebuilds a local `DMG-07` session for 2, 3, or 4 players by cloning the current P1 ROM into the selected adapter slots. Original `SUPER GB` keeps `EXT. PORT` disabled because the hardware has no physical Game Link port; `SUPER GB2` retains its physical link support.

The CGB `IR` menu is visible only for `CONFIG -> SYSTEM -> MODEL GB COLOR`; `MODEL GB ADVANCE` remains CGB-family for GB/C execution but has no physical CGB infrared port and does not show the IR menu. It supports `NONE`, CGB-to-CGB `SAME GAME`, CGB-to-CGB `SELECT GAME`, `PIKACHU 2`, and `MYSTERY GIFT`; accessory modes expose gift selectors and an optional `IR -> HELPER ON/OFF` overlay status. CGB IR keeps serial `EXT. PORT` attachments at `NONE` and treats optical sessions as separate from cable-link mode.

Pocket Camera controls are frontend-owned. `CAM LIVE` opens the first SDL3 camera, converts frames to grayscale, mirrors live frames for self-facing orientation, and feeds the same core API used by `CAM IMAGE`; `CAM RESET` stops live capture, clears the session image, and restores the deterministic placeholder. Still-image path and live-camera state are session-scoped and are not persisted.

## Battery saves and state files

Desktop cartridge persistence defaults to debounced auto-flush: changed persistence is written through a safe replacement file after roughly `2s`, and pending changes are forced on ROM changes and shutdown. `CONFIG -> SYSTEM -> SAVE` owns export/import, manual save, saves on/off, policy, automatic/custom directory, and save directory selection; `SAVE BATTERY` is visible only when the policy is `manual`.

Default save filenames preserve the active ROM's exact filename stem. External-stable P1 cartridges use `<stem>.sav`; local multi-cartridge slots use `<stem>.sa2`, `<stem>.sa3`, and `<stem>.sa4`; internal fallback slots use `<stem>.gbsav`, `<stem>.gbsa2`, `<stem>.gbsa3`, and `<stem>.gbsa4`. Same-ROM linked sessions still get independent player-slot files, and CGB IR `SELECT GAME` uses the secondary ROM stem for P2.

`EXPORT SAVE` writes the current primary/P1 cartridge persistence as an external-emulator-compatible `.sav` when the mapper has a safe external layout. `IMPORT SAVE` validates the selected external file against the current primary/P1 ROM, writes the authoritative runtime save, and asks the user to reload or reset; it does not hot-swap the live cartridge session.

MBC3 RTC-backed cartridges release a suspend-aware `32.768 kHz` RTC tick budget on the active emulation T-cycle cadence instead of batching ticks only at host event or save boundaries. Export/import flushes pending RTC ticks before serializing persistence.

Full-machine `.gbstate` files and rewind snapshots are separate from cartridge saves. State slots/autoload are single-machine features and are disabled for local `DMG-04`, local CGB IR pairs, `PokemonPikachuColor`, `PokemonMysteryGift`, and `DMG-07` linked sessions.

## Settings persistence

Desktop settings are stored under the platform config directory unless `GB_CYCLE_DESKTOP_SETTINGS_PATH` points to a specific settings file.

Persisted settings include launch `model`/startup/mode, boot-ROM path/verification, save policy/directory/enabled state, state autoload slot, rewind/Fast Forward policy, video presentation settings, audio device policy, input/gamepad bindings, preferred SDL gamepad identity, last opened directory, and recent-ROM history. The persisted `model` key uses the same values as `--model`: `DMG`, `MGB`, `LGB`, `CGB`, `AGB`, `SGB`, and `SGB2`.

Audio channel masks and automatic `RECORD` state are intentionally not persisted, so a fresh launch starts with the full mix selected and recording disabled unless CLI recording flags explicitly request otherwise. Pocket Camera still-image and live-camera state are also not persisted.

## Release packages

Desktop release workflows package only the SDL3 frontend. Each platform workflow builds `gb-desktop` with `cargo build --profile release-max -p gb-desktop --features gb-desktop/sdl3-build-from-source` and validates the packaged runtime layout by running `gb-desktop --help` before upload.

| Workflow | Package | Contents |
| --- | --- | --- |
| `release-windows.yml` | `gb-cycle-windows-x86_64.zip` | `gb-desktop.exe`, [`../../README.md`](../../README.md), both license files, and emitted `.dll` files. |
| `release-linux.yml` | `gb-cycle-linux-x86_64.tar.gz` | `gb-desktop`, [`../../README.md`](../../README.md), both license files, bundled `libSDL3*.so*` when emitted, and `$ORIGIN` rpath handling for dynamic SDL3. |
| `release-macos.yml` | `gb-cycle-macos-aarch64.zip` | `GB Cycle.app`, [`../../README.md`](../../README.md), both license files, bundled `libSDL3*.dylib` under `Contents/MacOS` when emitted, ad-hoc signing, and the same camera usage string as the local macOS launcher. |

Manual platform workflow runs validate/upload artifacts with `actions/upload-artifact`. Tag pushes matching `v*` attach those artifacts to the GitHub Release. The repository-level `release.yml` workflow owns the PAT-backed SemVer release flow: create/update `codex/release-<version>`, open the release PR, wait for required PR checks, squash-merge, create the annotated `v<version>` tag, create the GitHub Release, and dispatch `rom-reports-pages.yml` from the release tag so GitHub Pages can publish ROM reports for the same revision. The checkout step uses `github.token` for the initial source read without persisting checkout credentials; preflight fetches explicitly authenticate read-only with `github.token`, while branch and tag pushes authenticate with `RELEASE_PAT`.

## Error handling boundaries

Desktop load/runtime failures that need user action use native SDL3 message boxes; diagnostics, skipped-mode reasons, profiler output, trace setup errors, and investigation telemetry remain on `stderr` or explicit artifact files.

When `CONFIG -> SYSTEM -> START REAL` is selected but the configured boot-ROM directory or active model-specific dump no longer exists, desktop falls back to `skip-boot` for that session instead of aborting startup. CLI and ROM-suite RealBoot validation remain stricter and should be used for accuracy claims.
