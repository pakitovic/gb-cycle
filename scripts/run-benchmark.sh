#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
benchmark_dir="$script_dir/benchmark"

action_init=false
rom_dir=""
run_cli=true
run_desktop=true
single_test=""

usage() {
  cat <<'EOF'
Usage:
  scripts/run-benchmark.sh --init
  scripts/run-benchmark.sh --rom-dir <dir>
  scripts/run-benchmark.sh [--no-cli] [--no-desktop] [--test test/game.toml]

Options:
  --init              Create only scripts/benchmark/test/ and test/game.toml placeholder if missing.
  --rom-dir <dir>     Rewrite rom = "..." in scripts/benchmark/test/*.toml preserving each ROM basename.
  --no-cli            Run only gb-desktop.
  --no-desktop        Run only gb-cli.
  --test <path>       Run one benchmark case and regenerate scripts/benchmark/index.html.
  -h, --help          Show this help.

Benchmark cases and outputs live under scripts/benchmark/. Set GB_CYCLE_REPO_ROOT to override repo discovery.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --init)
      action_init=true
      shift
      ;;
    --rom-dir)
      if [[ $# -lt 2 ]]; then
        echo "error: --rom-dir requires a value" >&2
        exit 2
      fi
      rom_dir="$2"
      shift 2
      ;;
    --no-cli)
      run_cli=false
      shift
      ;;
    --no-desktop)
      run_desktop=false
      shift
      ;;
    --test)
      if [[ $# -lt 2 ]]; then
        echo "error: --test requires a value" >&2
        exit 2
      fi
      single_test="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

init_benchmark_tree() {
  mkdir -p "$benchmark_dir/test"
  local placeholder="$benchmark_dir/test/game.toml"
  if [[ ! -e "$placeholder" ]]; then
    cat > "$placeholder" <<'EOF'
version = 1
id = "game"
rom = "/roms/game.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
palette = "grey"
duration_seconds = 8
screenshot = true
stats = true

[[run]]
id = "idle-8"
label = "8s idle"
duration_seconds = 8

[[run]]
id = "start-a-8"
label = "8s Start/A"
duration_seconds = 8

[[run.input]]
frame = 30
button = "start"
hold_frames = 8
repeat_every_frames = 60

[[run.input]]
frame = 60
button = "a"
hold_frames = 8
repeat_every_frames = 60
EOF
  fi
  echo "initialized $benchmark_dir/test"
}

rewrite_rom_dir() {
  local target_dir="$1"
  if [[ -z "$target_dir" ]]; then
    echo "error: --rom-dir requires a directory" >&2
    exit 2
  fi
  if [[ ! -d "$benchmark_dir/test" ]]; then
    echo "error: $benchmark_dir/test does not exist; run --init first" >&2
    exit 1
  fi
  python3 - "$benchmark_dir" "$target_dir" <<'PY'
import json
import pathlib
import re
import sys

benchmark_dir = pathlib.Path(sys.argv[1])
rom_dir = pathlib.Path(sys.argv[2]).expanduser()
test_dir = benchmark_dir / "test"
cases = sorted(test_dir.glob("*.toml"))
if not cases:
    print(f"error: no benchmark cases found in {test_dir}", file=sys.stderr)
    sys.exit(1)
pattern = re.compile(r'^(\s*rom\s*=\s*)(["\'])(.*?)(\2)(\s*(?:#.*)?)$')
updated = 0
for case_path in cases:
    text = case_path.read_text()
    lines = text.splitlines(keepends=True)
    changed = False
    next_lines = []
    for line in lines:
        line_body = line[:-1] if line.endswith('\n') else line
        newline = '\n' if line.endswith('\n') else ''
        match = pattern.match(line_body)
        if match and not changed:
            current_rom = match.group(3)
            basename = pathlib.PurePosixPath(current_rom).name or pathlib.Path(current_rom).name
            if not basename:
                print(f"warning: {case_path} has an empty ROM basename; skipped", file=sys.stderr)
                next_lines.append(line)
                continue
            next_rom = rom_dir / basename
            next_lines.append(f"{match.group(1)}{json.dumps(str(next_rom))}{match.group(5)}{newline}")
            changed = True
            updated += 1
        else:
            next_lines.append(line)
    if changed:
        case_path.write_text(''.join(next_lines))
        print(f"updated {case_path.relative_to(benchmark_dir)}")
    else:
        print(f"warning: no rom = entry found in {case_path.relative_to(benchmark_dir)}", file=sys.stderr)
print(f"updated {updated} benchmark case(s)")
PY
}

find_repo_root() {
  local candidates=()
  if [[ -n "${GB_CYCLE_REPO_ROOT:-}" ]]; then
    candidates+=("$GB_CYCLE_REPO_ROOT")
  fi
  candidates+=("$script_dir" "$PWD")
  for candidate in "${candidates[@]}"; do
    local dir
    dir="$(cd "$candidate" 2>/dev/null && pwd || true)"
    while [[ -n "$dir" && "$dir" != "/" ]]; do
      if [[ -f "$dir/Cargo.toml" && -d "$dir/crates/gb-cli" && -d "$dir/crates/gb-desktop" ]]; then
        printf '%s\n' "$dir"
        return 0
      fi
      dir="$(dirname "$dir")"
    done
  done
  return 1
}

resolve_single_test() {
  local requested="$1"
  if [[ "$requested" = /* ]]; then
    printf '%s\n' "$requested"
    return 0
  fi

  local benchmark_relative="$benchmark_dir/$requested"
  if [[ -f "$benchmark_relative" ]]; then
    printf '%s\n' "$benchmark_relative"
    return 0
  fi

  local cwd_relative="$PWD/$requested"
  if [[ -f "$cwd_relative" ]]; then
    printf '%s\n' "$cwd_relative"
    return 0
  fi

  printf '%s\n' "$benchmark_relative"
}

collect_cases() {
  if [[ -n "$single_test" ]]; then
    local candidate
    candidate="$(resolve_single_test "$single_test")"
    if [[ ! -f "$candidate" ]]; then
      echo "error: benchmark test not found: $single_test" >&2
      exit 1
    fi
    printf '%s\n' "$candidate"
    return 0
  fi

  if [[ ! -d "$benchmark_dir/test" ]]; then
    echo "error: no benchmark cases found; $benchmark_dir/test does not exist (run --init first)" >&2
    exit 1
  fi
  local found=false
  shopt -s nullglob
  local case_path
  for case_path in "$benchmark_dir"/test/*.toml; do
    found=true
    printf '%s\n' "$case_path"
  done
  shopt -u nullglob
  if [[ "$found" == false ]]; then
    echo "error: no benchmark cases found in $benchmark_dir/test" >&2
    exit 1
  fi
}

case_label() {
  python3 - "$benchmark_dir" "$1" <<'PY'
import pathlib
import sys
root = pathlib.Path(sys.argv[1]).resolve()
path = pathlib.Path(sys.argv[2]).resolve()
try:
    print(path.relative_to(root))
except ValueError:
    print(path.name)
PY
}

generate_index() {
  python3 - "$benchmark_dir" <<'PY'
from __future__ import annotations

import html
import pathlib
import sys
from datetime import datetime

benchmark_dir = pathlib.Path(sys.argv[1])
try:
    import tomllib
except ModuleNotFoundError:
    tomllib = None


def parse_scalar(value: str):
    value = value.strip()
    if not value:
        return ""
    if value.startswith('"'):
        try:
            import json
            return json.loads(value)
        except Exception:
            return value.strip('"')
    if value.startswith("'") and value.endswith("'"):
        return value[1:-1]
    if value in {"true", "false"}:
        return value == "true"
    try:
        return int(value)
    except ValueError:
        pass
    try:
        return float(value)
    except ValueError:
        return value


def load_toml(path: pathlib.Path) -> dict:
    if not path.exists():
        return {}
    if tomllib is not None:
        try:
            with path.open('rb') as f:
                return tomllib.load(f)
        except Exception:
            return {}
    data = {}
    for raw_line in path.read_text().splitlines():
        line = raw_line.split('#', 1)[0].strip()
        if not line or line.startswith('[') or '=' not in line:
            continue
        key, value = line.split('=', 1)
        data[key.strip()] = parse_scalar(value)
    return data


def fmt_number(value, digits=2):
    try:
        return f"{float(value):.{digits}f}"
    except (TypeError, ValueError):
        return "—"


def rel(path: pathlib.Path) -> str:
    return path.relative_to(benchmark_dir).as_posix()

cases = []
for case_path in sorted((benchmark_dir / 'test').glob('*.toml')):
    data = load_toml(case_path)
    case_id = data.get('id') or case_path.stem
    cases.append((case_id, case_path, data))

def expanded_runs(case_id, data):
    runs = data.get('run')
    if isinstance(runs, list) and runs:
        for index, run in enumerate(runs, start=1):
            run_id = str(run.get('id') or f'run{index}')
            yield {
                'artifact_id': f'{case_id}-{run_id}',
                'run': run.get('label') or run_id,
                'seconds': run.get('duration_seconds', data.get('duration_seconds', '—')),
            }
    else:
        yield {
            'artifact_id': str(case_id),
            'run': 'default',
            'seconds': data.get('duration_seconds', '—'),
        }

rows = []
for case_id, case_path, data in cases:
    for run in expanded_runs(case_id, data):
        artifact_id = run['artifact_id']
        cells = [
            html.escape(str(case_id)),
            html.escape(str(run['run'])),
            html.escape(case_path.relative_to(benchmark_dir).as_posix()),
            html.escape(str(data.get('model', '—'))),
            html.escape(str(run['seconds'])),
        ]
        for frontend in ('gb-cli', 'gb-desktop'):
            stats_path = benchmark_dir / frontend / f'{artifact_id}-stats.toml'
            stats = load_toml(stats_path)
            image_path = benchmark_dir / frontend / f'{artifact_id}.png'
            if stats:
                metrics = f"{fmt_number(stats.get('fps'))} FPS<br>{fmt_number(stats.get('speed_percent'), 1)}%"
            else:
                metrics = '—'
            if image_path.exists():
                image = f'<a href="{html.escape(rel(image_path))}"><img src="{html.escape(rel(image_path))}" alt="{html.escape(frontend)} {html.escape(str(artifact_id))}" loading="lazy"></a>'
            else:
                image = '—'
            cells.append(metrics)
            cells.append(image)
        rows.append('<tr>' + ''.join(f'<td>{cell}</td>' for cell in cells) + '</tr>')

if not rows:
    rows.append('<tr><td colspan="9">No benchmark cases found in test/*.toml.</td></tr>')

index = f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>gb-cycle benchmark</title>
<style>
body {{ font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; margin: 2rem; color: #111; }}
table {{ border-collapse: collapse; width: 100%; }}
th, td {{ border: 1px solid #ccc; padding: .5rem; vertical-align: top; }}
th {{ background: #f4f4f4; position: sticky; top: 0; }}
img {{ width: 160px; height: 144px; image-rendering: pixelated; background: #ddd; }}
code {{ background: #f4f4f4; padding: .1rem .25rem; }}
.meta {{ color: #555; }}
</style>
</head>
<body>
<h1>gb-cycle benchmark</h1>
<p class="meta">Generated {html.escape(datetime.now().isoformat(timespec='seconds'))}. Commands: <code>gb-cli run --test-runner --benchmark &lt;case&gt;</code> and <code>gb-desktop --test-runner --benchmark &lt;case&gt;</code>.</p>
<table>
<thead>
<tr><th>id</th><th>run</th><th>case</th><th>model</th><th>seconds</th><th>gb-cli</th><th>gb-cli screenshot</th><th>gb-desktop</th><th>gb-desktop screenshot</th></tr>
</thead>
<tbody>
{''.join(rows)}
</tbody>
</table>
</body>
</html>
'''
(benchmark_dir / 'index.html').write_text(index)
print(f"wrote {benchmark_dir / 'index.html'}")
PY
}

if [[ "$action_init" == true ]]; then
  init_benchmark_tree
  exit 0
fi

if [[ -n "$rom_dir" ]]; then
  rewrite_rom_dir "$rom_dir"
  exit 0
fi

if [[ "$run_cli" == false && "$run_desktop" == false ]]; then
  echo "error: --no-cli and --no-desktop disable all frontends" >&2
  exit 2
fi

cases=()
while IFS= read -r case_path; do
  cases+=("$case_path")
done < <(collect_cases)

repo_root="$(find_repo_root || true)"
if [[ -z "$repo_root" ]]; then
  echo "error: could not find gb-cycle repo root; set GB_CYCLE_REPO_ROOT" >&2
  exit 1
fi

(
  cd "$repo_root"
  cargo build --profile release-max -p gb-cli -p gb-desktop
)

if [[ "$run_cli" == true ]]; then
  mkdir -p "$benchmark_dir/gb-cli"
fi
if [[ "$run_desktop" == true ]]; then
  mkdir -p "$benchmark_dir/gb-desktop"
fi

gb_cli_bin="${GB_CLI_BIN:-$repo_root/target/release-max/gb-cli}"
gb_desktop_bin="${GB_DESKTOP_BIN:-$repo_root/target/release-max/gb-desktop}"

for case_path in "${cases[@]}"; do
  echo "==> $(case_label "$case_path")"
  if [[ "$run_cli" == true ]]; then
    echo "--> gb-cli"
    (cd "$benchmark_dir" && "$gb_cli_bin" run --test-runner --benchmark "$case_path")
  fi
  if [[ "$run_desktop" == true ]]; then
    echo "--> gb-desktop"
    (cd "$benchmark_dir" && "$gb_desktop_bin" --test-runner --benchmark "$case_path")
  fi
done

generate_index
