# gb-core test layout

This directory owns integration tests that exercise the public `gb-core` API and cross-module wiring while staying independent of CLI, desktop, web, audio, file-dialog, or other frontend crates.

## Layout

- `*.rs`: integration-test entry points compiled as separate crates.
- `common/`: shared integration-test helpers for fixture lookup, machine driving, and synthetic cartridge construction.
- `fixtures/roms/`: small core-owned synthetic ROM fixtures used by `gb-core` tests.
- `fixtures/traces/`: golden trace artifacts and text snapshots used by deterministic core tests.

Prefer unit tests next to local pure logic under `src/`; use this directory when behavior depends on the public API, the top-level `Machine`, or multiple subsystems. Phase-specific fixture README files are retained only where they name exact shipped assets, source-of-truth builders, or regeneration commands. Project-wide validation policy and fixture ownership rules live in [docs/TESTING.md](../../../docs/TESTING.md).
