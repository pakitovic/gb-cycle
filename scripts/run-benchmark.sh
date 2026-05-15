#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
benchmark_dir="$script_dir/benchmark"

action_sample=false
rom_dir=""
run_cli=false
single_test=""
case_dir=""

usage() {
  cat <<'USAGE'
Usage:
  scripts/run-benchmark.sh --sample
  scripts/run-benchmark.sh <case-dir> [--rom-dir <rom-dir>]
  scripts/run-benchmark.sh <case-dir> [--gb-cli] [--test <case.toml>]

Arguments:
  <case-dir>        Directory containing benchmark case *.toml files.

Options:
  --sample          Create game.toml sample next to run-benchmark.sh if missing.
  --rom-dir <dir>   Rewrite rom = "..." in <case-dir>/*.toml preserving each ROM basename.
  --gb-cli          Run gb-cli in addition to the default gb-desktop benchmark.
  --test <path>     Run one benchmark case; relative paths resolve against <case-dir>, then $PWD.
  -h, --help        Show this help.

Outputs are written to benchmark/ next to run-benchmark.sh. By default only gb-desktop runs.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sample)
      action_sample=true
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
    --gb-cli)
      run_cli=true
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
    --*)
      echo "error: unknown option $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ -n "$case_dir" ]]; then
        echo "error: unexpected argument $1" >&2
        usage >&2
        exit 2
      fi
      case_dir="$1"
      shift
      ;;
  esac
done

write_sample_case() {
  local sample="$script_dir/game.toml"
  if [[ ! -e "$sample" ]]; then
    cat > "$sample" <<'SAMPLE'
version = 1
id = "game"
rom = "/roms/game.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
palette = "grey"
screenshot = true
stats = true

[[run]]
id = "idle-40"
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
frame = 60
button = "a"
hold_frames = 8
repeat_every_frames = 60
SAMPLE
    echo "wrote $sample"
  else
    echo "sample already exists: $sample"
  fi
}

resolve_existing_dir() {
  python3 - "$1" "$2" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]).expanduser()
label = sys.argv[2]
if not path.is_dir():
    print(f"error: {label} is not a directory: {path}", file=sys.stderr)
    sys.exit(1)
print(path.resolve())
PY
}

rewrite_rom_dir() {
  local target_case_dir="$1"
  local target_rom_dir="$2"
  if [[ -z "$target_rom_dir" ]]; then
    echo "error: --rom-dir requires a directory" >&2
    exit 2
  fi
  python3 - "$target_case_dir" "$target_rom_dir" <<'PY'
import json
import pathlib
import re
import sys

case_dir = pathlib.Path(sys.argv[1]).resolve()
rom_dir = pathlib.Path(sys.argv[2]).expanduser()
cases = sorted(case_dir.glob("*.toml"))
if not cases:
    print(f"error: no benchmark cases found in {case_dir}", file=sys.stderr)
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
            basename = pathlib.PureWindowsPath(current_rom).name or pathlib.PurePosixPath(current_rom).name or pathlib.Path(current_rom).name
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
        print(f"updated {case_path.relative_to(case_dir)}")
    else:
        print(f"warning: no rom = entry found in {case_path.relative_to(case_dir)}", file=sys.stderr)
print(f"updated {updated} benchmark case(s)")
PY
}

find_repo_root() {
  local candidates=("$script_dir" "$PWD")
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

  local case_relative="$case_dir/$requested"
  if [[ -f "$case_relative" ]]; then
    printf '%s\n' "$case_relative"
    return 0
  fi

  local cwd_relative="$PWD/$requested"
  if [[ -f "$cwd_relative" ]]; then
    printf '%s\n' "$cwd_relative"
    return 0
  fi

  printf '%s\n' "$case_relative"
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

  local found=false
  shopt -s nullglob
  local case_path
  for case_path in "$case_dir"/*.toml; do
    found=true
    printf '%s\n' "$case_path"
  done
  shopt -u nullglob
  if [[ "$found" == false ]]; then
    echo "error: no benchmark cases found in $case_dir" >&2
    exit 1
  fi
}

case_label() {
  python3 - "$case_dir" "$1" <<'PY'
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
  python3 - "$benchmark_dir" "$case_dir" "$run_cli" <<'PY'
from __future__ import annotations

import html
import pathlib
import sys
from datetime import datetime

benchmark_dir = pathlib.Path(sys.argv[1]).resolve()
case_dir = pathlib.Path(sys.argv[2]).resolve()
include_cli = sys.argv[3] == 'true'
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
    target = data
    for raw_line in path.read_text().splitlines():
        line = raw_line.split('#', 1)[0].strip()
        if not line:
            continue
        if line.startswith('[[') and line.endswith(']]'):
            table = line[2:-2].strip()
            if table == 'run':
                run = {}
                data.setdefault('run', []).append(run)
                target = run
            else:
                target = None
            continue
        if line.startswith('['):
            target = None
            continue
        if target is None or '=' not in line:
            continue
        key, value = line.split('=', 1)
        target[key.strip()] = parse_scalar(value)
    return data


def fmt_number(value, digits=2):
    try:
        return f"{float(value):.{digits}f}"
    except (TypeError, ValueError):
        return "—"


def rel(path: pathlib.Path) -> str:
    return path.relative_to(benchmark_dir).as_posix()


def case_rel(path: pathlib.Path) -> str:
    try:
        return path.relative_to(case_dir).as_posix()
    except ValueError:
        return path.name


def rom_name(value) -> str:
    if value is None:
        return "—"
    text = str(value)
    if not text:
        return "—"
    return pathlib.PureWindowsPath(text).name or pathlib.PurePosixPath(text).name or text


def table_cell(content: str, rowspan: int = 1) -> str:
    span = f' rowspan="{rowspan}"' if rowspan > 1 else ""
    return f"<td{span}>{content}</td>"


FRONTENDS = ('gb-cli', 'gb-desktop') if include_cli else ('gb-desktop',)


cases = []
for case_path in sorted(case_dir.glob('*.toml')):
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
                'seconds': run.get('duration_seconds', '—'),
            }
    else:
        yield {
            'artifact_id': str(case_id),
            'seconds': '—',
        }


def frontend_artifacts(artifact_id):
    artifacts = {}
    for frontend in FRONTENDS:
        stats_path = benchmark_dir / frontend / f'{artifact_id}-stats.toml'
        image_path = benchmark_dir / frontend / f'{artifact_id}.png'
        stats = load_toml(stats_path)
        has_complete_artifacts = bool(stats) and image_path.exists()
        artifacts[frontend] = {
            'stats': stats if has_complete_artifacts else {},
            'image_path': image_path if has_complete_artifacts else None,
            'has_complete_artifacts': has_complete_artifacts,
        }
    return artifacts


rows = []
for case_id, case_path, data in cases:
    runs = []
    for run in expanded_runs(case_id, data):
        artifacts = frontend_artifacts(run['artifact_id'])
        if not any(artifact['has_complete_artifacts'] for artifact in artifacts.values()):
            continue
        run['artifacts'] = artifacts
        runs.append(run)
    if not runs:
        continue
    case_rowspan = len(runs)
    case_cells = [
        html.escape(rom_name(data.get('rom'))),
        html.escape(case_rel(case_path)),
        html.escape(str(data.get('model', '—'))),
    ]
    for run_index, run in enumerate(runs):
        artifact_id = run['artifact_id']
        cells = []
        if run_index == 0:
            cells.extend(table_cell(cell, case_rowspan) for cell in case_cells)
        cells.append(table_cell(html.escape(str(run['seconds']))))
        for frontend in FRONTENDS:
            artifact = run['artifacts'][frontend]
            stats = artifact['stats']
            image_path = artifact['image_path']
            if stats:
                metrics = f"{fmt_number(stats.get('fps'))} FPS<br>{fmt_number(stats.get('speed_percent'), 1)}%"
            else:
                metrics = '—'
            if image_path is not None:
                image = f'<a href="{html.escape(rel(image_path))}"><img src="{html.escape(rel(image_path))}" alt="{html.escape(frontend)} {html.escape(str(artifact_id))}" loading="lazy"></a>'
            else:
                image = '—'
            cells.append(table_cell(metrics))
            cells.append(table_cell(image))
        rows.append('<tr>' + ''.join(cells) + '</tr>')

column_count = 4 + (len(FRONTENDS) * 2)
if not rows:
    rows.append(f'<tr><td colspan="{column_count}">No executed benchmark artifacts found. Run scripts/run-benchmark.sh &lt;case-dir&gt; first.</td></tr>')

frontend_headers = ''.join(f'<th>{html.escape(frontend)}</th><th>{html.escape(frontend)} screenshot</th>' for frontend in FRONTENDS)
commands = []
if include_cli:
    commands.append('<code>gb-cli run --test-runner --benchmark &lt;case&gt;</code>')
commands.append('<code>gb-desktop --test-runner --benchmark &lt;case&gt;</code>')
command_text = ' and '.join(commands) if len(commands) == 2 else commands[0]

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
td[rowspan] {{ vertical-align: middle; }}
th {{ background: #f4f4f4; position: sticky; top: 0; }}
img {{ width: 160px; height: 144px; image-rendering: pixelated; background: #ddd; }}
code {{ background: #f4f4f4; padding: .1rem .25rem; }}
.meta {{ color: #555; }}
</style>
</head>
<body>
<h1>gb-cycle benchmark</h1>
<p class="meta">Generated {html.escape(datetime.now().isoformat(timespec='seconds'))}. Case directory: <code>{html.escape(str(case_dir))}</code>. Command: {command_text}.</p>
<table>
<thead>
<tr><th>rom</th><th>case</th><th>model</th><th>seconds</th>{frontend_headers}</tr>
</thead>
<tbody>
{''.join(rows)}
</tbody>
</table>
</body>
</html>
'''
benchmark_dir.mkdir(parents=True, exist_ok=True)
(benchmark_dir / 'index.html').write_text(index)
print(f"wrote {benchmark_dir / 'index.html'}")
PY
}

if [[ "$action_sample" == true ]]; then
  if [[ -n "$case_dir" || -n "$rom_dir" || -n "$single_test" || "$run_cli" == true ]]; then
    echo "error: --sample cannot be combined with benchmark run options" >&2
    exit 2
  fi
  write_sample_case
  exit 0
fi

if [[ -z "$case_dir" ]]; then
  echo "error: <case-dir> is required" >&2
  usage >&2
  exit 2
fi

case_dir="$(resolve_existing_dir "$case_dir" "<case-dir>")"

if [[ -n "$rom_dir" ]]; then
  if [[ -n "$single_test" || "$run_cli" == true ]]; then
    echo "error: --rom-dir cannot be combined with benchmark run options" >&2
    exit 2
  fi
  rewrite_rom_dir "$case_dir" "$rom_dir"
  exit 0
fi

cases=()
while IFS= read -r case_path; do
  cases+=("$case_path")
done < <(collect_cases)

repo_root="$(find_repo_root || true)"
if [[ -z "$repo_root" ]]; then
  echo "error: could not find gb-cycle repo root from the script directory or current directory" >&2
  exit 1
fi

cargo_packages=(-p gb-desktop)
if [[ "$run_cli" == true ]]; then
  cargo_packages=(-p gb-cli -p gb-desktop)
fi

(
  cd "$repo_root"
  cargo build --profile release-max "${cargo_packages[@]}"
)

mkdir -p "$benchmark_dir/gb-desktop"
if [[ "$run_cli" == true ]]; then
  mkdir -p "$benchmark_dir/gb-cli"
fi

gb_cli_bin="${GB_CLI_BIN:-$repo_root/target/release-max/gb-cli}"
gb_desktop_bin="${GB_DESKTOP_BIN:-$repo_root/target/release-max/gb-desktop}"

for case_path in "${cases[@]}"; do
  echo "==> $(case_label "$case_path")"
  if [[ "$run_cli" == true ]]; then
    echo "--> gb-cli"
    (cd "$benchmark_dir" && "$gb_cli_bin" run --test-runner --benchmark "$case_path")
  fi
  echo "--> gb-desktop"
  (cd "$benchmark_dir" && "$gb_desktop_bin" --test-runner --benchmark "$case_path")
done

generate_index
