#!/usr/bin/env bash
set -eu

if [ -z "${GB_CYCLE_SAMEBOY_ROOT:-}" ]; then
    echo "GB_CYCLE_SAMEBOY_ROOT is not set." >&2
    exit 2
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TEST_ROM_ROOT="${GB_CYCLE_TEST_ROM_ROOT:-$REPO_ROOT/.roms/test}"
ROM_ROOT="$TEST_ROM_ROOT/docboy"
RUNNER="${GB_CYCLE_SAMEBOY_CASE_BUNDLE_BIN:-$GB_CYCLE_SAMEBOY_ROOT/build/bin/gb_cycle_case_bundle_runner}"
OUT=".oracles/sameboy/docboy-protocol-baseline.md"
FILTER=""
TIMEOUT_FRAMES=80
ROM_LIST_FILE=""
BOOT_ROM_DMG=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --rom-root) ROM_ROOT="$2"; shift 2 ;;
        --runner) RUNNER="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        --filter) FILTER="$2"; shift 2 ;;
        --rom-list) ROM_LIST_FILE="$2"; shift 2 ;;
        --timeout-frames) TIMEOUT_FRAMES="$2"; shift 2 ;;
        --boot-rom-root) BOOT_ROM_DMG="$2/dmg_boot.bin"; shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

if [ -n "$BOOT_ROM_DMG" ] && [ ! -f "$BOOT_ROM_DMG" ]; then
    echo "DMG boot ROM not found: $BOOT_ROM_DMG" >&2
    exit 2
fi

SAMEBOY_ROOT="$GB_CYCLE_SAMEBOY_ROOT"
HELPER_SRC="$REPO_ROOT/crates/gb-test-runner/c_support/sameboy_case_bundle_runner.c"

build_runner_if_missing() {
    if [ -x "$RUNNER" ] && [ "$RUNNER" -nt "$HELPER_SRC" ]; then
        return 0
    fi
    if [ ! -f "$SAMEBOY_ROOT/build/lib/libsameboy.o" ]; then
        echo "Missing $SAMEBOY_ROOT/build/lib/libsameboy.o (run 'make lib' in SameBoy)." >&2
        return 1
    fi
    mkdir -p "$(dirname "$RUNNER")"
    cc -std=c11 -O2 \
        -I "$SAMEBOY_ROOT/Core" \
        -o "$RUNNER" \
        "$HELPER_SRC" \
        "$SAMEBOY_ROOT/build/lib/libsameboy.o" \
        -lm
}

if ! build_runner_if_missing; then
    exit 2
fi

if [ ! -d "$ROM_ROOT" ]; then
    echo "DocBoy ROM root not found: $ROM_ROOT" >&2
    echo "Run 'make fetch-test-roms FAMILIES=docboy' to materialize it from upstream." >&2
    exit 2
fi

mkdir -p "$(dirname "$OUT")"

REPORT_TITLE="# SameBoy baseline for docboy memory-protocol ROMs"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

ROM_LIST="$TMP_DIR/roms.txt"
if [ -n "$ROM_LIST_FILE" ]; then
    if [ ! -f "$ROM_LIST_FILE" ]; then
        echo "ROM list file not found: $ROM_LIST_FILE" >&2
        exit 2
    fi
    cp "$ROM_LIST_FILE" "$ROM_LIST"
else
    ( cd "$ROM_ROOT" && find . -type f -name '*.gb' ) \
      | sed 's|^\./||' | sort > "$ROM_LIST"
fi

if [ -n "$FILTER" ]; then
    grep -E "$FILTER" "$ROM_LIST" > "$TMP_DIR/filtered.txt" || true
    mv "$TMP_DIR/filtered.txt" "$ROM_LIST"
fi

TOTAL=$(wc -l < "$ROM_LIST" | tr -d ' ')
PASS=0
FAIL=0
MISSING=0

{
    echo "$REPORT_TITLE"
    echo
    echo "- Runner: \`$RUNNER\`"
    echo "- ROM root: \`$ROM_ROOT\`"
    if [ -n "$BOOT_ROM_DMG" ]; then
        echo "- Startup: RealBoot (\`$BOOT_ROM_DMG\`)"
    else
        echo "- Startup: SkipBoot (synthetic)"
    fi
    echo "- Total ROMs: $TOTAL"
    echo "- Timeout: ${TIMEOUT_FRAMES} frames"
    echo "- HRAM check: address 0xFFF0, pass value 1"
    echo
    echo "| rom | sameboy |"
    echo "| --- | --- |"
} > "$OUT"

while IFS= read -r rom; do
    full="$ROM_ROOT/$rom"
    if [ ! -f "$full" ]; then
        MISSING=$((MISSING + 1))
        echo "| $rom | ℹ️ |" >> "$OUT"
        continue
    fi
    line=$("$RUNNER" \
        --model dmg \
        --rom "$full" \
        ${BOOT_ROM_DMG:+--boot-rom "$BOOT_ROM_DMG"} \
        --check-hram 0xFFF0 1 \
        --timeout-frames "$TIMEOUT_FRAMES" \
        </dev/null 2>/dev/null \
      | grep '^RESULT' | tail -1)
    if echo "$line" | grep -q '^RESULT pass'; then
        PASS=$((PASS + 1))
        echo "| $rom | ✅ |" >> "$OUT"
    elif echo "$line" | grep -q '^RESULT fail'; then
        FAIL=$((FAIL + 1))
        actual=$(echo "$line" | sed -n 's/.*value=\([0-9]*\).*/\1/p')
        echo "| $rom | ❌ (got=$actual) |" >> "$OUT"
    else
        FAIL=$((FAIL + 1))
        echo "| $rom | ❌ (no result line) |" >> "$OUT"
    fi
done < "$ROM_LIST"

SUMMARY="Summary: ✅ $PASS / ❌ $FAIL / ℹ️ $MISSING (out of $TOTAL)"
sed -i.bak "1a\\
\\
**$SUMMARY**
" "$OUT"
rm -f "${OUT}.bak"

echo "$SUMMARY"
echo "Report written to: $OUT"
