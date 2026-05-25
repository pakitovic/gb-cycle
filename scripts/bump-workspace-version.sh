#!/usr/bin/env bash
# Bump every gb-cycle workspace crate to one SemVer release version.
# shellcheck disable=SC2034,SC2154

set -euo pipefail

export LC_ALL=C

usage() {
  cat <<'USAGE'
Usage: bump-workspace-version.sh [--dry-run] [--check] [--allow-same] [--print-normalized] VERSION

Bump every workspace crate package version, internal workspace dependency version,
and Cargo.lock workspace package version to VERSION.

With --check, validate that the workspace already matches VERSION without
modifying files.

VERSION accepts SemVer MAJOR.MINOR.PATCH with an optional leading v and optional
prerelease suffix, for example 0.1.7, v0.1.7, or 0.1.7-rc.1.
USAGE
}

error() {
  printf 'error: %s\n' "$*" >&2
}

is_numeric_identifier() {
  [[ $1 =~ ^[0-9]+$ ]]
}

parse_semver_into() {
  local raw=$1
  local prefix=$2
  local semver_re='^v?([0-9]+)\.([0-9]+)\.([0-9]+)(-([0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*))?$'

  raw=${raw//$'\r'/}
  raw=${raw#"${raw%%[![:space:]]*}"}
  raw=${raw%"${raw##*[![:space:]]}"}

  if [[ ! $raw =~ $semver_re ]]; then
    error 'expected SemVer MAJOR.MINOR.PATCH with optional prerelease suffix, for example 0.1.7 or 0.1.7-rc.1'
    return 2
  fi

  local major=${BASH_REMATCH[1]}
  local minor=${BASH_REMATCH[2]}
  local patch=${BASH_REMATCH[3]}
  local prerelease=${BASH_REMATCH[5]:-}

  local numeric
  for numeric in "$major" "$minor" "$patch"; do
    if [[ ${#numeric} -gt 1 && ${numeric:0:1} == 0 ]]; then
      error 'SemVer numeric identifiers must not contain leading zeroes'
      return 2
    fi
  done

  if [[ -n $prerelease ]]; then
    local old_ifs=$IFS
    local part
    IFS=.
    for part in $prerelease; do
      if [[ -z $part ]]; then
        IFS=$old_ifs
        error 'SemVer prerelease identifiers must not be empty'
        return 2
      fi
      if is_numeric_identifier "$part" && [[ ${#part} -gt 1 && ${part:0:1} == 0 ]]; then
        IFS=$old_ifs
        error 'SemVer numeric prerelease identifiers must not contain leading zeroes'
        return 2
      fi
    done
    IFS=$old_ifs
  fi

  local text="${major}.${minor}.${patch}"
  if [[ -n $prerelease ]]; then
    text="${text}-${prerelease}"
  fi

  printf -v "${prefix}_major" '%s' "$major"
  printf -v "${prefix}_minor" '%s' "$minor"
  printf -v "${prefix}_patch" '%s' "$patch"
  printf -v "${prefix}_prerelease" '%s' "$prerelease"
  printf -v "${prefix}_text" '%s' "$text"
}

semver_field() {
  local prefix=$1
  local field=$2
  local name="${prefix}_${field}"
  printf '%s' "${!name}"
}

compare_prerelease() {
  local left=$1
  local right=$2
  local left_parts=()
  local right_parts=()
  local old_ifs=$IFS
  IFS=.
  # shellcheck disable=SC2206
  left_parts=($left)
  # shellcheck disable=SC2206
  right_parts=($right)
  IFS=$old_ifs

  local left_count=${#left_parts[@]}
  local right_count=${#right_parts[@]}
  local count=$left_count
  if (( right_count < count )); then
    count=$right_count
  fi

  local index
  for (( index = 0; index < count; index += 1 )); do
    local left_part=${left_parts[$index]}
    local right_part=${right_parts[$index]}
    if is_numeric_identifier "$left_part" && is_numeric_identifier "$right_part"; then
      if (( 10#$left_part < 10#$right_part )); then
        printf '%s\n' -1
        return 0
      fi
      if (( 10#$left_part > 10#$right_part )); then
        printf '%s\n' 1
        return 0
      fi
    elif is_numeric_identifier "$left_part"; then
      printf '%s\n' -1
      return 0
    elif is_numeric_identifier "$right_part"; then
      printf '%s\n' 1
      return 0
    elif [[ $left_part < $right_part ]]; then
      printf '%s\n' -1
      return 0
    elif [[ $left_part > $right_part ]]; then
      printf '%s\n' 1
      return 0
    fi
  done

  if (( left_count < right_count )); then
    printf '%s\n' -1
  elif (( left_count > right_count )); then
    printf '%s\n' 1
  else
    printf '%s\n' 0
  fi
}

compare_semver() {
  local left=$1
  local right=$2
  local field
  for field in major minor patch; do
    local left_value
    local right_value
    left_value=$(semver_field "$left" "$field")
    right_value=$(semver_field "$right" "$field")
    if (( 10#$left_value < 10#$right_value )); then
      printf '%s\n' -1
      return 0
    fi
    if (( 10#$left_value > 10#$right_value )); then
      printf '%s\n' 1
      return 0
    fi
  done

  local left_prerelease
  local right_prerelease
  left_prerelease=$(semver_field "$left" prerelease)
  right_prerelease=$(semver_field "$right" prerelease)

  if [[ -z $left_prerelease && -n $right_prerelease ]]; then
    printf '%s\n' 1
  elif [[ -n $left_prerelease && -z $right_prerelease ]]; then
    printf '%s\n' -1
  elif [[ -z $left_prerelease && -z $right_prerelease ]]; then
    printf '%s\n' 0
  else
    compare_prerelease "$left_prerelease" "$right_prerelease"
  fi
}

repo_root() {
  local script_dir
  script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
  cd -- "${script_dir}/.." && pwd
}

parse_package_manifest() {
  local manifest=$1
  awk '
    /^\[package\]$/ { in_package = 1; next }
    in_package && /^\[/ { in_package = 0 }
    in_package && /^[[:space:]]*name[[:space:]]*=/ {
      value = $0
      sub(/^[^"]*"/, "", value)
      sub(/".*/, "", value)
      name = value
    }
    in_package && /^[[:space:]]*version[[:space:]]*=/ {
      value = $0
      sub(/^[^"]*"/, "", value)
      sub(/".*/, "", value)
      version = value
    }
    END {
      if (name == "") {
        printf("error: %s: missing package name\n", FILENAME) > "/dev/stderr"
        exit 2
      }
      if (version == "") {
        printf("error: %s: missing package version\n", FILENAME) > "/dev/stderr"
        exit 2
      }
      printf("%s\t%s\n", name, version)
    }
  ' "$manifest"
}

escape_awk_regex() {
  printf '%s' "$1" | sed 's/[][\\.^$*+?{}()|]/\\&/g'
}

join_crate_regex() {
  local first=1
  local name
  local escaped
  for name in "$@"; do
    escaped=$(escape_awk_regex "$name")
    if (( first )); then
      printf '%s' "$escaped"
      first=0
    else
      printf '|%s' "$escaped"
    fi
  done
}

changed_paths=()

tmp_file_for() {
  local base=$1
  mktemp "${TMPDIR:-/tmp}/${base}.XXXXXX"
}

bump_manifest() {
  local manifest=$1
  local crate_re=$2
  local version=$3
  local dry_run=$4
  local tmp
  tmp=$(tmp_file_for bump-manifest)

  if ! awk -v version="$version" -v crate_re="$crate_re" '
    BEGIN {
      in_package = 0
      package_version_replaced = 0
      dependency_error = 0
      dependency_re = "^[[:space:]]*(" crate_re ")[[:space:]]*=[[:space:]]*\\{"
    }
    /^\[package\]$/ {
      in_package = 1
      print
      next
    }
    in_package && /^\[/ { in_package = 0 }
    {
      line = $0
      if (in_package && package_version_replaced == 0 && line ~ /^[[:space:]]*version[[:space:]]*=/) {
        line = "version = \"" version "\""
        package_version_replaced = 1
      }
      if (line ~ dependency_re && line ~ /path[[:space:]]*=/) {
        if (line !~ /version[[:space:]]*=/) {
          printf("error: %s: internal dependency line is missing a version: %s\n", FILENAME, line) > "/dev/stderr"
          dependency_error = 1
        } else {
          sub(/version[[:space:]]*=[[:space:]]*"[^"]+"/, "version = \"" version "\"", line)
        }
      }
      print line
    }
    END {
      if (package_version_replaced == 0) {
        printf("error: %s: missing package version\n", FILENAME) > "/dev/stderr"
        exit 2
      }
      if (dependency_error != 0) {
        exit 2
      }
    }
  ' "$manifest" > "$tmp"; then
    rm -f -- "$tmp"
    return 2
  fi

  if cmp -s -- "$manifest" "$tmp"; then
    rm -f -- "$tmp"
    return 0
  fi

  changed_paths+=("$manifest")
  if [[ $dry_run == false ]]; then
    mv -- "$tmp" "$manifest"
  else
    rm -f -- "$tmp"
  fi
}

bump_lockfile() {
  local lockfile=$1
  local crate_re=$2
  local version=$3
  local dry_run=$4
  local tmp
  tmp=$(tmp_file_for bump-lockfile)

  if ! awk -v version="$version" -v crate_re="$crate_re" '
    BEGIN { current_package = ""; crate_name_re = "^(" crate_re ")$" }
    /^\[\[package\]\]$/ {
      current_package = ""
      print
      next
    }
    {
      line = $0
      if (line ~ /^name[[:space:]]*=/) {
        value = line
        sub(/^[^"]*"/, "", value)
        sub(/".*/, "", value)
        current_package = value
      } else if (current_package ~ crate_name_re && line ~ /^version[[:space:]]*=/) {
        line = "version = \"" version "\""
      }
      print line
    }
  ' "$lockfile" > "$tmp"; then
    rm -f -- "$tmp"
    return 2
  fi

  if cmp -s -- "$lockfile" "$tmp"; then
    rm -f -- "$tmp"
    return 0
  fi

  changed_paths+=("$lockfile")
  if [[ $dry_run == false ]]; then
    mv -- "$tmp" "$lockfile"
  else
    rm -f -- "$tmp"
  fi
}

dry_run=false
check=false
allow_same=false
print_normalized=false
version_input=

while (($# > 0)); do
  case $1 in
    --dry-run)
      dry_run=true
      ;;
    --check)
      check=true
      dry_run=true
      ;;
    --allow-same)
      allow_same=true
      ;;
    --print-normalized)
      print_normalized=true
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --*)
      error "unknown option: $1"
      usage >&2
      exit 2
      ;;
    *)
      if [[ -n $version_input ]]; then
        error "unexpected extra argument: $1"
        usage >&2
        exit 2
      fi
      version_input=$1
      ;;
  esac
  shift
done

if [[ -z $version_input ]]; then
  error 'missing VERSION'
  usage >&2
  exit 2
fi

if ! parse_semver_into "$version_input" target; then
  exit 2
fi

if [[ $print_normalized == true ]]; then
  printf '%s\n' "$target_text"
  exit 0
fi

root=$(repo_root)
cd -- "$root"

shopt -s nullglob
manifests=("$root"/crates/*/Cargo.toml)
shopt -u nullglob

if (( ${#manifests[@]} == 0 )); then
  error 'no crate manifests found under crates/*/Cargo.toml'
  exit 2
fi

crate_names=()
current_text=
details=

for manifest in "${manifests[@]}"; do
  package_line=$(parse_package_manifest "$manifest") || exit 2
  package_name=${package_line%%$'\t'*}
  package_version=${package_line#*$'\t'}

  if ! parse_semver_into "$package_version" package; then
    error "${manifest}: invalid package version ${package_version}"
    exit 2
  fi

  crate_names+=("$package_name")
  details+="  ${manifest}: ${package_text}"$'\n'

  if [[ -z $current_text ]]; then
    current_text=$package_text
    current_major=$package_major
    current_minor=$package_minor
    current_patch=$package_patch
    current_prerelease=$package_prerelease
  elif [[ $(compare_semver current package) != 0 ]]; then
    error "workspace crate versions are not aligned:"$'\n'"${details%$'\n'}"
    exit 2
  fi
done

comparison=$(compare_semver target current)
if [[ $comparison == -1 || ( $comparison == 0 && $allow_same == false ) ]]; then
  if [[ $comparison == 0 ]]; then
    comparator='equal to'
  else
    comparator='older than'
  fi
  error "target version ${target_text} is ${comparator} current workspace version ${current_text}"
  exit 2
fi

crate_re=$(join_crate_regex "${crate_names[@]}")

for manifest in "${manifests[@]}"; do
  bump_manifest "$manifest" "$crate_re" "$target_text" "$dry_run"
done

lockfile="$root/Cargo.lock"
if [[ ! -f $lockfile ]]; then
  error 'missing Cargo.lock'
  exit 2
fi
bump_lockfile "$lockfile" "$crate_re" "$target_text" "$dry_run"

if [[ $check == true ]]; then
  if (( ${#changed_paths[@]} > 0 )); then
    printf 'Workspace crate versions do not match %s; would update:\n' "$target_text" >&2
    for path in "${changed_paths[@]}"; do
      printf '  %s\n' "${path#"$root"/}" >&2
    done
    exit 1
  fi

  printf 'Workspace crate versions match %s\n' "$target_text"
  exit 0
elif [[ $dry_run == true ]]; then
  action='Would update'
else
  action='Updated'
fi

if (( ${#changed_paths[@]} > 0 )); then
  printf '%s workspace crate versions from %s to %s:\n' "$action" "$current_text" "$target_text"
  for path in "${changed_paths[@]}"; do
    printf '  %s\n' "${path#"$root"/}"
  done
else
  printf 'Workspace crate versions already match %s\n' "$target_text"
fi
