#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
benchmark_dir="$script_dir/benchmark"

action_sample=false
rom_dir=""
run_cli=false
single_test=""
case_dir=""
normalize_case=false
generate_cases=false
template_path=""

usage() {
  cat <<'USAGE'
Usage:
  scripts/run-benchmark.sh --sample
  scripts/run-benchmark.sh <case-dir> [--rom-dir <rom-dir>]
  scripts/run-benchmark.sh <case-dir> --normalize-case
  scripts/run-benchmark.sh <case-dir> --rom-dir <rom-dir> --generate-cases [--template <case.toml>]
  scripts/run-benchmark.sh [<case-dir>] [--gb-cli] --test <case.toml>

Arguments:
  <case-dir>        Directory containing benchmark case *.toml files; optional with --test.

Options:
  --sample          Create game.toml sample next to run-benchmark.sh if missing.
  --rom-dir <dir>   Rewrite rom = "..." in <case-dir>/*.toml preserving each ROM basename.
  --normalize-case  Rename <case-dir>/*.toml from each case's ROM filename stem.
  --generate-cases  Generate normalized cases for every *.gb and *.gbc ROM under --rom-dir.
  --template <path>  Use a benchmark case TOML template with --generate-cases.
  --gb-cli          Run gb-cli in addition to the default gb-desktop benchmark.
  --test <path>     Run one benchmark case; without <case-dir>, infer it from this file.
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
    --normalize-case)
      normalize_case=true
      shift
      ;;
    --generate-cases)
      generate_cases=true
      shift
      ;;
    --template)
      if [[ $# -lt 2 ]]; then
        echo "error: --template requires a value" >&2
        exit 2
      fi
      template_path="$2"
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

resolve_or_create_dir() {
  python3 - "$1" "$2" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]).expanduser()
label = sys.argv[2]
try:
    path.mkdir(parents=True, exist_ok=True)
except OSError as error:
    print(f"error: failed to create {label} {path}: {error}", file=sys.stderr)
    sys.exit(1)
if not path.is_dir():
    print(f"error: {label} is not a directory: {path}", file=sys.stderr)
    sys.exit(1)
print(path.resolve())
PY
}

resolve_existing_test_path() {
  python3 - "$1" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]).expanduser()
if not path.is_absolute():
    path = pathlib.Path.cwd() / path
if not path.is_file():
    print(f"error: benchmark test not found: {sys.argv[1]}", file=sys.stderr)
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

normalize_cases() {
  local target_case_dir="$1"
  python3 - "$target_case_dir" <<'PY'
import json
import pathlib
import re
import sys

case_dir = pathlib.Path(sys.argv[1]).resolve()
cases = sorted(case_dir.glob("*.toml"), key=lambda path: path.name.casefold())
if not cases:
    print(f"error: no benchmark cases found in {case_dir}", file=sys.stderr)
    sys.exit(1)

assignment = re.compile(r'^(\s*)([A-Za-z0-9_-]+)(\s*=\s*)(.*?)(\s*(?:#.*)?)$')


def parse_toml_string(value: str) -> str:
    value = value.strip()
    if value.startswith('"'):
        try:
            return json.loads(value)
        except Exception:
            return value.strip('"')
    if value.startswith("'") and value.endswith("'"):
        return value[1:-1]
    return value.split('#', 1)[0].strip()


def top_level_value(path: pathlib.Path, key: str):
    in_table = False
    for raw_line in path.read_text().splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith('#'):
            continue
        if stripped.startswith('['):
            in_table = True
            continue
        if in_table:
            continue
        match = assignment.match(raw_line)
        if match and match.group(2) == key:
            return parse_toml_string(match.group(4))
    return None


def portable_name(value: str) -> str:
    windows = pathlib.PureWindowsPath(value).name
    posix = pathlib.PurePosixPath(value).name
    if '\\' in value and windows:
        return windows
    return posix or windows or pathlib.Path(value).name


def normalized_case_name(rom: str) -> str:
    basename = portable_name(rom)
    stem = pathlib.PurePosixPath(basename).stem
    return f"{stem}.toml"


renamed = 0
unchanged = 0
skipped = 0
errors = 0
for case_path in cases:
    rom = top_level_value(case_path, 'rom')
    if not rom:
        print(f"warning: no top-level rom = entry found in {case_path.name}; skipped", file=sys.stderr)
        skipped += 1
        continue
    target_path = case_dir / normalized_case_name(rom)
    if case_path.name == target_path.name:
        unchanged += 1
        print(f"unchanged {case_path.name}")
        continue
    if target_path.exists():
        try:
            same_file = case_path.samefile(target_path)
        except OSError:
            same_file = False
        if same_file:
            temp_path = case_dir / f".{case_path.name}.normalize-{os.getpid()}.tmp"
            case_path.rename(temp_path)
            temp_path.rename(target_path)
            renamed += 1
            print(f"renamed {case_path.name} -> {target_path.name}")
            continue
        print(f"error: cannot rename {case_path.name} to {target_path.name}; target already exists", file=sys.stderr)
        errors += 1
        continue
    case_path.rename(target_path)
    renamed += 1
    print(f"renamed {case_path.name} -> {target_path.name}")
if errors:
    sys.exit(1)
print(f"renamed {renamed}, unchanged {unchanged}, skipped {skipped} benchmark case(s)")
PY
}

generate_benchmark_cases() {
  local target_case_dir="$1"
  local target_rom_dir="$2"
  local target_template_path="$3"
  if [[ -z "$target_rom_dir" ]]; then
    echo "error: --generate-cases requires --rom-dir" >&2
    exit 2
  fi
  python3 - "$target_case_dir" "$target_rom_dir" "$target_template_path" <<'PY'
import json
import pathlib
import re
import sys
import unicodedata

case_dir = pathlib.Path(sys.argv[1]).resolve()
rom_dir = pathlib.Path(sys.argv[2]).expanduser()
template_arg = sys.argv[3]

DEFAULT_TEMPLATE = """version = 1
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
"""

if not rom_dir.is_dir():
    print(f"error: --rom-dir is not a directory: {rom_dir}", file=sys.stderr)
    sys.exit(1)

if template_arg:
    template_path = pathlib.Path(template_arg).expanduser()
    if not template_path.is_file():
        print(f"error: --template is not a file: {template_path}", file=sys.stderr)
        sys.exit(1)
    template_text = template_path.read_text()
else:
    template_text = DEFAULT_TEMPLATE

roms = sorted(
    (
        path
        for path in rom_dir.rglob('*')
        if path.is_file() and path.suffix.lower() in {'.gb', '.gbc'}
    ),
    key=lambda path: str(path).casefold(),
)
if not roms:
    print(f"error: no .gb or .gbc ROMs found in {rom_dir}", file=sys.stderr)
    sys.exit(1)

assignment = re.compile(r'^(\s*)(id|rom|model)(\s*=\s*)(.*?)(\s*(?:#.*)?)$')


def model_for_rom(path: pathlib.Path) -> str:
    suffix = path.suffix.lower()
    if suffix == '.gb':
        return 'DMG'
    if suffix == '.gbc':
        return 'CGB'
    raise AssertionError(f"unsupported ROM suffix {path.suffix}")


def safe_id(stem: str) -> str:
    ascii_stem = unicodedata.normalize('NFKD', stem).encode('ascii', 'ignore').decode('ascii')
    slug = re.sub(r'[^A-Za-z0-9_-]+', '-', ascii_stem).strip('-_').lower()
    return slug or 'game'


def json_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def render_case(template: str, rom_path: pathlib.Path) -> str:
    replacements = {
        'id': safe_id(rom_path.stem),
        'rom': str(rom_path.resolve()),
        'model': model_for_rom(rom_path),
    }
    found = set()
    lines = template.splitlines(keepends=True)
    output = []
    in_table = False
    first_table_index = None
    for line in lines:
        stripped = line.strip()
        if stripped.startswith('['):
            in_table = True
            if first_table_index is None:
                first_table_index = len(output)
        if not in_table:
            newline = '\n' if line.endswith('\n') else ''
            body = line[:-1] if newline else line
            match = assignment.match(body)
            if match:
                key = match.group(2)
                found.add(key)
                output.append(f"{match.group(1)}{key} = {json_string(replacements[key])}{match.group(5)}{newline}")
                continue
        output.append(line)
    missing = [key for key in ('id', 'rom', 'model') if key not in found]
    if missing:
        insert_at = first_table_index if first_table_index is not None else len(output)
        if insert_at > 0 and output and not output[insert_at - 1].endswith('\n'):
            output[insert_at - 1] += '\n'
        insert_lines = [f"{key} = {json_string(replacements[key])}\n" for key in missing]
        output[insert_at:insert_at] = insert_lines
    text = ''.join(output)
    if text and not text.endswith('\n'):
        text += '\n'
    return text


targets = {}
errors = 0
for rom_path in roms:
    target_path = case_dir / f"{rom_path.stem}.toml"
    previous = targets.get(target_path)
    if previous is not None:
        print(f"error: ROMs {previous} and {rom_path} both normalize to {target_path.name}", file=sys.stderr)
        errors += 1
    targets[target_path] = rom_path
if errors:
    sys.exit(1)

created = 0
updated = 0
unchanged = 0
for target_path, rom_path in sorted(targets.items(), key=lambda item: item[0].name.casefold()):
    rendered = render_case(template_text, rom_path)
    if target_path.exists():
        if target_path.read_text() == rendered:
            unchanged += 1
            print(f"unchanged {target_path.name}")
            continue
        target_path.write_text(rendered)
        updated += 1
        print(f"updated {target_path.name}")
    else:
        target_path.write_text(rendered)
        created += 1
        print(f"wrote {target_path.name}")
print(f"created {created}, updated {updated}, unchanged {unchanged} benchmark case(s)")
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

filter_valid_cases() {
  python3 - "$@" <<'PY'
import json
import os
import pathlib
import re
import sys

try:
    import tomllib
except ModuleNotFoundError:
    tomllib = None

assignment = re.compile(r'^(\s*)([A-Za-z0-9_-]+)(\s*=\s*)(.*?)(\s*(?:#.*)?)$')


def parse_toml_string(value):
    value = value.strip()
    if value.startswith('"'):
        try:
            return json.loads(value)
        except Exception:
            return value.strip('"')
    if value.startswith("'") and value.endswith("'"):
        return value[1:-1]
    return value.split('#', 1)[0].strip()


def fallback_top_level_rom(path):
    in_table = False
    for raw_line in path.read_text().splitlines():
        stripped = raw_line.strip()
        if not stripped or stripped.startswith('#'):
            continue
        if stripped.startswith('['):
            in_table = True
            continue
        if in_table:
            continue
        match = assignment.match(raw_line)
        if match and match.group(2) == 'rom':
            return parse_toml_string(match.group(4))
    return None


def case_rom(path):
    if tomllib is not None:
        try:
            with path.open('rb') as file:
                data = tomllib.load(file)
        except Exception as error:
            return None, f"invalid TOML: {error}"
        rom = data.get('rom')
        if not isinstance(rom, str) or not rom:
            return None, "missing top-level string rom"
        return rom, None
    try:
        rom = fallback_top_level_rom(path)
    except Exception as error:
        return None, f"failed to read TOML: {error}"
    if not rom:
        return None, "missing top-level string rom"
    return rom, None


def resolve_rom(case_path, rom):
    try:
        rom_path = pathlib.Path(rom).expanduser()
    except RuntimeError as error:
        return None, f"cannot expand ROM path {rom!r}: {error}"
    if not rom_path.is_absolute():
        rom_path = case_path.parent / rom_path
    return rom_path, None


def validate_rom(case_path):
    rom, error = case_rom(case_path)
    if error is not None:
        return None, error
    rom_path, error = resolve_rom(case_path, rom)
    if error is not None:
        return None, error
    if not rom_path.is_file():
        return rom_path, "ROM does not exist or is not a file"
    try:
        size = rom_path.stat().st_size
    except OSError as error:
        return rom_path, f"cannot stat ROM: {error}"
    if size <= 0:
        return rom_path, "ROM is empty"
    try:
        with rom_path.open('rb') as file:
            file.read(1)
    except OSError as error:
        return rom_path, f"cannot read ROM: {error}"
    return rom_path.resolve(), None


total = 0
valid = 0
skipped = 0
for raw_case in sys.argv[1:]:
    total += 1
    case_path = pathlib.Path(raw_case).expanduser()
    if not case_path.is_absolute():
        case_path = pathlib.Path.cwd() / case_path
    try:
        case_path = case_path.resolve()
    except OSError:
        case_path = case_path.absolute()
    rom_path, error = validate_rom(case_path)
    if error is not None:
        rom_display = f" ({rom_path})" if rom_path is not None else ""
        print(f"warning: skipping {case_path.name}{rom_display}: {error}", file=sys.stderr)
        skipped += 1
        continue
    print(case_path)
    valid += 1

print(f"validated {valid}/{total} benchmark case ROM(s); skipped {skipped}", file=sys.stderr)
PY
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
  if [[ -n "$case_dir" || -n "$rom_dir" || -n "$single_test" || "$run_cli" == true || "$normalize_case" == true || "$generate_cases" == true || -n "$template_path" ]]; then
    echo "error: --sample cannot be combined with benchmark run options" >&2
    exit 2
  fi
  write_sample_case
  exit 0
fi

if [[ "$normalize_case" == true && ( -n "$rom_dir" || -n "$single_test" || "$run_cli" == true || "$generate_cases" == true || -n "$template_path" ) ]]; then
  echo "error: --normalize-case cannot be combined with other benchmark actions" >&2
  exit 2
fi

if [[ -n "$template_path" && "$generate_cases" != true ]]; then
  echo "error: --template requires --generate-cases" >&2
  exit 2
fi

if [[ "$generate_cases" == true && -z "$rom_dir" ]]; then
  echo "error: --generate-cases requires --rom-dir" >&2
  exit 2
fi

if [[ "$generate_cases" == true && ( -n "$single_test" || "$run_cli" == true ) ]]; then
  echo "error: --generate-cases cannot be combined with benchmark run options" >&2
  exit 2
fi

if [[ -n "$rom_dir" && ( -n "$single_test" || "$run_cli" == true ) ]]; then
  echo "error: --rom-dir cannot be combined with benchmark run options" >&2
  exit 2
fi

if [[ -z "$case_dir" ]]; then
  if [[ -n "$single_test" ]]; then
    single_test="$(resolve_existing_test_path "$single_test")"
    case_dir="$(dirname "$single_test")"
  else
    echo "error: <case-dir> is required" >&2
    usage >&2
    exit 2
  fi
else
  if [[ "$generate_cases" == true ]]; then
    case_dir="$(resolve_or_create_dir "$case_dir" "<case-dir>")"
  else
    case_dir="$(resolve_existing_dir "$case_dir" "<case-dir>")"
  fi
fi

if [[ "$normalize_case" == true ]]; then
  normalize_cases "$case_dir"
  exit 0
fi

if [[ "$generate_cases" == true ]]; then
  generate_benchmark_cases "$case_dir" "$rom_dir" "$template_path"
  exit 0
fi

if [[ -n "$rom_dir" ]]; then
  rewrite_rom_dir "$case_dir" "$rom_dir"
  exit 0
fi

cases=()
while IFS= read -r case_path; do
  cases+=("$case_path")
done < <(collect_cases)

valid_cases=()
while IFS= read -r case_path; do
  valid_cases+=("$case_path")
done < <(filter_valid_cases "${cases[@]}")
cases=()
if [[ "${#valid_cases[@]}" -gt 0 ]]; then
  cases=("${valid_cases[@]}")
fi

if [[ "${#cases[@]}" -eq 0 ]]; then
  echo "warning: no benchmark cases with readable ROMs found; nothing to run" >&2
  generate_index
  exit 0
fi

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
