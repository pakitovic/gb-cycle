# gb-core test layout

This directory owns automated tests that exercise the public `gb-core` API and
cross-module wiring.

## Layout

- `*.rs`: integration tests compiled as separate crates
- `common/`: shared helpers for fixture lookup and future harness code
- `fixtures/roms/`: ROM fixtures for automated harnesses
- `fixtures/traces/`: golden trace artifacts and text snapshots

## Phase 0 baseline

- Keep fast, deterministic smoke coverage here even before the scheduler exists.
- Prefer unit tests in `src/` for local pure logic.
- Prefer integration tests here when behavior depends on the public API or
  multiple modules.
- Do not make these tests depend on `gb-cli` or any frontend crate.
