# gb-benchmark

`gb-benchmark` owns the portable benchmark case contract shared by `gb-cli`, `gb-desktop`, and the `cargo rom-bench` batch helper. It is a tooling layer over the normal `gb-core` hardware model: benchmark cases choose the ROM, model, startup path, compatibility mode, duration, and deterministic joypad inputs, while each frontend remains responsible for its own host work, screenshots, and timing stats.

Use benchmark runs for repeatable frontend/core performance comparisons and screenshot-backed local reports. Do not treat a benchmark case as a hardware oracle; use ROM-suite validation for correctness claims and benchmark results only after the relevant behavior already has tests or oracle coverage.

## Batch workflow

`cargo rom-bench` runs from the workspace and writes all batch artifacts under `test/bench/`. By default it builds and runs `gb-desktop` with `--profile release-max`; add `--gb-cli` only when you want matching headless artifacts and report columns.

```bash
# Create test/bench/game.toml if it does not already exist
cargo rom-bench --sample

# Run every *.toml benchmark case in a directory through gb-desktop
cargo rom-bench path/to/benchmark-cases

# Run gb-cli first and then gb-desktop for each case, including both columns in the HTML report
cargo rom-bench path/to/benchmark-cases --gb-cli

# Run a single case; when <case-dir> is omitted, it is inferred from the case file
cargo rom-bench --test path/to/benchmark-cases/game.toml
cargo rom-bench path/to/benchmark-cases --gb-cli --test game.toml
```

The batch runner validates that each case parses and that its resolved ROM path exists, is non-empty, and is readable. Cases with unreadable ROMs are skipped with warnings; if no valid cases remain, `test/bench/index.html` is still regenerated from whatever complete artifacts already exist.

## Case maintenance helpers

| Command | Purpose |
| --- | --- |
| `cargo rom-bench --sample` | Writes `test/bench/game.toml` using the default portable case template if it is missing. |
| `cargo rom-bench <case-dir> --rom-dir <rom-dir>` | Rewrites the top-level `rom = "..."` path in each case to point at `<rom-dir>/<existing-rom-basename>`. |
| `cargo rom-bench <case-dir> --normalize-case` | Renames each case file from the top-level ROM filename stem, preserving the `.toml` extension. |
| `cargo rom-bench <case-dir> --rom-dir <rom-dir> --generate-cases [--template <case.toml>]` | Recursively generates normalized cases for every `.gb` and `.gbc` ROM under `<rom-dir>`, using the default template or the provided template. |

Use the maintenance helpers when preparing a local benchmark directory from private ROMs or when moving the ROM root between machines. Keep generated private cases and ROMs out of version control unless the ROMs are redistributable fixtures.

## Case TOML format

A benchmark TOML file is versioned with `version = 1`, has one top-level ROM/config section, and one or more `[[run]]` entries. Relative `rom` paths resolve against the TOML file's directory, so portable case directories should keep ROM paths either relative to the case file or rewritten with `--rom-dir` for the local machine.

```toml
version = 1
id = "game"
rom = "roms/game.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
palette = "grey"
screenshot = true
stats = true

[[run]]
id = "idle-40"
label = "Idle for 40 seconds"
duration_seconds = 40

[[run]]
id = "start-a-120"
duration_seconds = 120

[[run.input]]
frame = 30
button = "start"
hold_frames = 8
repeat_every_frames = 60

[[run.input]]
second = 2
buttons = ["a", "b"]
hold_frames = 8

[[run.input]]
tcycle = 70224
button = "select"
hold_frames = 8
```

| Field | Meaning |
| --- | --- |
| `id` | Case artifact prefix; must contain only ASCII letters, digits, `-`, and `_`. |
| `rom` | ROM path for the benchmark suite; relative paths are resolved from the TOML directory. |
| `model` | One of `DMG`, `MGB`, `LGB`, or `CGB`. SGB/SGB2 benchmark cases are not part of this contract. |
| `startup` | One of `skip-boot`, `custom-boot`, or `real-boot`. |
| `mode` | One of `strict`, `permissive`, or `experimental`. |
| `palette` | Optional; currently `grey` and only meaningful when `model = "DMG"`. |
| `screenshot` / `stats` | Optional top-level defaults for all runs; both default to `true`. |
| `[[run]].id` | Per-run artifact suffix; required and filename-safe with the same character rules as top-level `id`. |
| `[[run]].label` | Optional display label stored in stats. |
| `[[run]].duration_seconds` | Required positive duration for that run. |
| `[[run]].screenshot` / `[[run]].stats` | Optional per-run overrides for the top-level artifact toggles. |

The current format intentionally rejects the old top-level `duration_seconds` plus `[[stimulus]]` layout. Put duration and inputs under each `[[run]]` entry so one TOML suite can describe multiple repeatable scenes for the same ROM.

## Deterministic inputs

Each `[[run.input]]` creates a press/release pulse. Define exactly one timing field, exactly one button selector, and an optional hold/repeat policy.

| Field | Rule |
| --- | --- |
| `frame` | Press when the frontend reports this completed-frame index. |
| `second` | Converted to the corresponding target DMG frame index. |
| `tcycle` | Press on this T-cycle; `repeat_every_frames` is not allowed with T-cycle timing. |
| `button` | One joypad button: `right`, `left`, `up`, `down`, `a`, `b`, `select`, or `start`. |
| `buttons` | Non-empty array of joypad buttons pressed and released together. |
| `hold_frames` | Optional hold length in frames; defaults to `8` and must be greater than zero. |
| `repeat_every_frames` | Optional frame-domain repeat interval; must be greater than `hold_frames`. |

Frame and second inputs are one-shot pulses by default. When `repeat_every_frames` is set, the same frame-domain pulse repeats until the run's target frame count is reached. T-cycle inputs are always one-shot pulses whose release time is `hold_frames * DMG_T_CYCLES_PER_FRAME` after the press.

## Direct frontend runs

Use direct frontend runs when iterating on one case without regenerating the batch HTML report. These commands write artifacts relative to the current working directory, so run them from `test/bench/` if you want the same layout that `cargo rom-bench` uses.

```bash
# Headless run through gb-cli
cargo run -p gb-cli -- run --test-runner --benchmark path/to/game.toml

# Desktop run through gb-desktop
cargo run --release -p gb-desktop -- --test-runner --benchmark path/to/game.toml
```

`--benchmark` supplies the ROM path from the TOML, so direct frontend commands must not also pass a positional ROM path. `--test-runner` applies automation-friendly frontend defaults; it does not replace the core timing model.

## Artifacts and report

Each expanded run uses artifact id `<case-id>-<run-id>`. Screenshots are written as `<frontend>/<artifact-id>.png`; stats are written as `<frontend>/<artifact-id>-stats.toml`. For batch runs those frontend directories live under `test/bench/`, and `test/bench/index.html` is regenerated after the run.

Stats TOML records the frontend, ROM, model, startup, mode, duration, target frames, completed frames, elapsed seconds, FPS, speed percentage, executed T-cycles when available, and the screenshot path when a screenshot was requested. The HTML index only shows rows with complete stats plus screenshot artifacts for the selected frontend columns.

## Interpretation rules

Compare like with like: keep the same Rust build profile, host machine, ROM files, case TOML, frontend set, `--test-runner` usage, and desktop host settings before treating FPS or speed deltas as meaningful. Prefer `release-max` batch runs for stable local comparisons, and use direct `--release` desktop runs only for quick iteration.

Benchmark input timing is deterministic, but host performance is not hardware behavior. If a benchmark exposes a correctness discrepancy, move the reduced behavior into an owning unit/integration/ROM-suite test before changing emulator semantics.
