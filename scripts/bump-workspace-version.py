#!/usr/bin/env python3
"""Bump every gb-cycle workspace crate to one SemVer release version."""

from __future__ import annotations

import argparse
import dataclasses
import pathlib
import re
import sys
from functools import total_ordering


SEMVER_RE = re.compile(
    r"^v?"
    r"(?P<major>0|[1-9][0-9]*)\."
    r"(?P<minor>0|[1-9][0-9]*)\."
    r"(?P<patch>0|[1-9][0-9]*)"
    r"(?:-(?P<prerelease>(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*))?"
    r"$"
)
PACKAGE_BLOCK_RE = re.compile(r"(?ms)^\[package\]\n(?P<body>.*?)(?=^\[|\Z)")
PACKAGE_NAME_RE = re.compile(r'(?m)^name\s*=\s*"([^"]+)"')
PACKAGE_VERSION_RE = re.compile(r'(?m)^version\s*=\s*"([^"]+)"')
LOCK_PACKAGE_START_RE = re.compile(r"^\[\[package\]\]$")
LOCK_NAME_RE = re.compile(r'^name\s*=\s*"([^"]+)"$')
LOCK_VERSION_RE = re.compile(r'^(version\s*=\s*)"([^"]+)"$')


@total_ordering
@dataclasses.dataclass(frozen=True)
class SemVer:
    major: int
    minor: int
    patch: int
    prerelease: tuple[str, ...]

    @property
    def text(self) -> str:
        base = f"{self.major}.{self.minor}.{self.patch}"
        if self.prerelease:
            return f"{base}-{'.'.join(self.prerelease)}"
        return base

    def __lt__(self, other: object) -> bool:
        if not isinstance(other, SemVer):
            return NotImplemented
        self_base = (self.major, self.minor, self.patch)
        other_base = (other.major, other.minor, other.patch)
        if self_base != other_base:
            return self_base < other_base
        if not self.prerelease and other.prerelease:
            return False
        if self.prerelease and not other.prerelease:
            return True
        return compare_prerelease(self.prerelease, other.prerelease) < 0

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, SemVer):
            return NotImplemented
        return (
            self.major == other.major
            and self.minor == other.minor
            and self.patch == other.patch
            and self.prerelease == other.prerelease
        )


def compare_prerelease(left: tuple[str, ...], right: tuple[str, ...]) -> int:
    for left_part, right_part in zip(left, right):
        left_numeric = left_part.isdigit()
        right_numeric = right_part.isdigit()
        if left_numeric and right_numeric:
            left_value = int(left_part)
            right_value = int(right_part)
            if left_value != right_value:
                return -1 if left_value < right_value else 1
        elif left_numeric != right_numeric:
            return -1 if left_numeric else 1
        elif left_part != right_part:
            return -1 if left_part < right_part else 1
    if len(left) == len(right):
        return 0
    return -1 if len(left) < len(right) else 1


def parse_semver(raw_version: str) -> SemVer:
    match = SEMVER_RE.fullmatch(raw_version.strip())
    if not match:
        raise ValueError(
            "expected SemVer MAJOR.MINOR.PATCH with optional prerelease suffix, "
            "for example 0.1.7 or 0.1.7-rc.1"
        )
    prerelease_raw = match.group("prerelease")
    prerelease = tuple(prerelease_raw.split(".")) if prerelease_raw else ()
    return SemVer(
        major=int(match.group("major")),
        minor=int(match.group("minor")),
        patch=int(match.group("patch")),
        prerelease=prerelease,
    )


def repo_root() -> pathlib.Path:
    return pathlib.Path(__file__).resolve().parents[1]


def crate_manifests(root: pathlib.Path) -> list[pathlib.Path]:
    return sorted(root.glob("crates/*/Cargo.toml"))


def parse_package_manifest(manifest: pathlib.Path) -> tuple[str, SemVer]:
    text = manifest.read_text()
    package_match = PACKAGE_BLOCK_RE.search(text)
    if not package_match:
        raise ValueError(f"{manifest}: missing [package] section")
    package_body = package_match.group("body")
    name_match = PACKAGE_NAME_RE.search(package_body)
    version_match = PACKAGE_VERSION_RE.search(package_body)
    if not name_match:
        raise ValueError(f"{manifest}: missing package name")
    if not version_match:
        raise ValueError(f"{manifest}: missing package version")
    return name_match.group(1), parse_semver(version_match.group(1))


def replace_package_version(text: str, version: str) -> tuple[str, bool]:
    package_match = PACKAGE_BLOCK_RE.search(text)
    if not package_match:
        raise ValueError("missing [package] section")

    package_block = package_match.group(0)
    updated_block, count = PACKAGE_VERSION_RE.subn(f'version = "{version}"', package_block, count=1)
    if count != 1:
        raise ValueError("missing package version")

    start, end = package_match.span()
    updated = text[:start] + updated_block + text[end:]
    return updated, updated != text


def replace_internal_dependency_versions(text: str, crate_names: set[str], version: str) -> tuple[str, bool]:
    changed = False
    updated_lines: list[str] = []
    dependency_line_re = re.compile(
        rf'^(?P<prefix>\s*(?:{"|".join(re.escape(name) for name in sorted(crate_names))})\s*=\s*\{{)(?P<body>.*)(?P<suffix>\}}\s*(?:#.*)?)$'
    )
    version_re = re.compile(r'(\bversion\s*=\s*)"[^"]+"')

    for line in text.splitlines(keepends=True):
        line_body = line[:-1] if line.endswith("\n") else line
        newline = "\n" if line.endswith("\n") else ""
        match = dependency_line_re.match(line_body)
        if match and "path" in match.group("body"):
            replacement, count = version_re.subn(rf'\1"{version}"', line_body, count=1)
            if count != 1:
                raise ValueError(f"internal dependency line is missing a version: {line_body}")
            if replacement != line_body:
                changed = True
                line_body = replacement
        updated_lines.append(f"{line_body}{newline}")

    updated = "".join(updated_lines)
    return updated, changed


def bump_manifest(manifest: pathlib.Path, crate_names: set[str], version: str, dry_run: bool) -> bool:
    text = manifest.read_text()
    updated, package_changed = replace_package_version(text, version)
    updated, dependency_changed = replace_internal_dependency_versions(updated, crate_names, version)
    changed = package_changed or dependency_changed
    if changed and not dry_run:
        manifest.write_text(updated)
    return changed


def bump_lockfile(lockfile: pathlib.Path, crate_names: set[str], version: str, dry_run: bool) -> bool:
    text = lockfile.read_text()
    changed = False
    current_package: str | None = None
    updated_lines: list[str] = []

    for line in text.splitlines(keepends=True):
        line_body = line[:-1] if line.endswith("\n") else line
        newline = "\n" if line.endswith("\n") else ""

        if LOCK_PACKAGE_START_RE.match(line_body):
            current_package = None
        else:
            name_match = LOCK_NAME_RE.match(line_body)
            if name_match:
                current_package = name_match.group(1)
            elif current_package in crate_names:
                version_match = LOCK_VERSION_RE.match(line_body)
                if version_match:
                    replacement = f'{version_match.group(1)}"{version}"'
                    if replacement != line_body:
                        line_body = replacement
                        changed = True

        updated_lines.append(f"{line_body}{newline}")

    if changed and not dry_run:
        lockfile.write_text("".join(updated_lines))
    return changed


def ensure_single_current_version(packages: list[tuple[pathlib.Path, str, SemVer]]) -> SemVer:
    versions = {package_version for _, _, package_version in packages}
    if len(versions) != 1:
        details = "\n".join(f"  {manifest}: {version.text}" for manifest, _, version in packages)
        raise ValueError(f"workspace crate versions are not aligned:\n{details}")
    return next(iter(versions))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help="Target SemVer version, for example 0.1.7 or v0.1.7")
    parser.add_argument("--dry-run", action="store_true", help="Print planned file updates without writing them")
    parser.add_argument(
        "--allow-same",
        action="store_true",
        help="Allow the target version to equal the current aligned workspace version",
    )
    parser.add_argument(
        "--print-normalized",
        action="store_true",
        help="Only validate the input and print the normalized version without a leading v",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        target = parse_semver(args.version)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    if args.print_normalized:
        print(target.text)
        return 0

    root = repo_root()
    manifests = crate_manifests(root)
    if not manifests:
        print("error: no crate manifests found under crates/*/Cargo.toml", file=sys.stderr)
        return 2

    try:
        packages = [(manifest, *parse_package_manifest(manifest)) for manifest in manifests]
        current = ensure_single_current_version(packages)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    if target < current or (target == current and not args.allow_same):
        comparator = "equal to" if target == current else "older than"
        print(
            f"error: target version {target.text} is {comparator} current workspace version {current.text}",
            file=sys.stderr,
        )
        return 2

    crate_names = {name for _, name, _ in packages}
    changed_paths: list[pathlib.Path] = []

    try:
        for manifest in manifests:
            if bump_manifest(manifest, crate_names, target.text, args.dry_run):
                changed_paths.append(manifest)
        lockfile = root / "Cargo.lock"
        if bump_lockfile(lockfile, crate_names, target.text, args.dry_run):
            changed_paths.append(lockfile)
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    action = "Would update" if args.dry_run else "Updated"
    if changed_paths:
        print(f"{action} workspace crate versions from {current.text} to {target.text}:")
        for path in changed_paths:
            print(f"  {path.relative_to(root)}")
    else:
        print(f"Workspace crate versions already match {target.text}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
