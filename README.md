# gb-cycle

A hardware-accuracy-focused Game Boy emulator written in Rust.

The core's base temporal unit is `1 T-cycle`, advanced on one shared scheduler timeline.
The target CPU foundation is a `fetch / decode / execute` core with real bus accesses.
The target graphics foundation is a `dot-by-dot` PPU with `tile fetcher + pixel FIFO`.

## Goals

- Prioritize behavior faithful to real hardware.
- Keep the core portable and decoupled from any frontend.
- Make validation through tests and reference ROMs straightforward.
- Build the core from the start on a `T-cycle` timeline, not an `M-cycle` timeline.
- Model the CPU from the start as a real fetch/decode/execute flow, not as opaque opcodes with aggregated duration.
- Model the PPU from the start as a real pipeline, not as a scanline renderer.

## Current structure

The canonical structure and ownership boundaries are defined in `AI/ARCHITECTURE.md`.
If this summary differs from `AI/ARCHITECTURE.md`, `AI/ARCHITECTURE.md` takes precedence.
The current workspace already uses the `crates/`-based layout, leaving other components as future extensions.

```text
crates/
  gb-core/    Pure emulation logic
  gb-cli/     Current CLI frontend
AI/           Architecture, roadmap, and technical documentation
Makefile      Local verification pipeline and utilities
```

Mid-term planned extensions, not yet materialized as separate crates:

- `gb-desktop`
- `gb-web`
- additional tooling such as a test runner, debugger, and utilities
- host persistence/save adapters outside the core, with any cartridge-facing persistence contract kept narrow and typed inside `gb-core`
- broader integration tests and ROM suites

### Requirements

- Rust `1.93.1` via `rustup`
- Workspace MSRV: `1.93`

## Tooling

This repository uses:

- `rustfmt` for formatting
- `clippy` for linting
- `cargo-llvm-cov` for coverage
- `cargo-deny` for dependency, advisory and license checks
- `typos` for spellchecking

### Install local tooling

```bash
make setup
```

`make setup` configures the repository git hooks and installs the required local cargo tools:

- `cargo-llvm-cov`
- `cargo-deny`
- `typos-cli`

### Coverage

```bash
cargo cov
cargo cov-html
cargo cov-lcov
```

### Full local pipeline

```bash
make check
make ci
```


## Documentation

Before implementing subsystems, read the main handbooks in `AI/` first:

- `AI/index.md`
- `AI/ARCHITECTURE.md`
- `AI/CODING-RULES.md`
- `AI/EXECUTION.md`
- `AI/REFERENCES.md`
- `AI/ROADMAP.md`
- `AI/TESTING.md`
- `AI/TIMING-AND-ACCURACY.md`
- `AI/hardware/*.md`

The documentation hierarchy, in summary, is:

- `AI/index.md` as the entry point for reading order and document authority boundaries
- `AI/ARCHITECTURE.md` for layout, ownership, and subsystem boundaries
- `AI/ARCHITECTURE.md` also for the central compatibility-policy structure and execution-mode ownership boundaries
- `AI/TIMING-AND-ACCURACY.md` for shared timing vocabulary and project-wide timing constraints
- `AI/ARCHITECTURE.md` plus `AI/TIMING-AND-ACCURACY.md` together for the global T-cycle scheduler contract
- `AI/EXECUTION.md` and `AI/CODING-RULES.md` for workflow and code-change discipline
- `AI/REFERENCES.md` for source and oracle consultation order
- `AI/hardware/*.md` for the behavior and contracts of the corresponding subsystem
- `AI/hardware/CARTRIDGES-MBC.md` specifically for mapper classification, special-cartridge taxonomy, compatibility-category policy, and cartridge persistence rules distinct from full emulator save states
- `AI/TESTING.md` for the global validation, differential, determinism, DMG-hardening policy, and official `Strict` CI/oracle usage
- `AI/ROADMAP.md` for recommended implementation order, phase context, and outstanding TODOs

Use `AI/research/*.md` as secondary comparison material when you need implementation examples, additional validation, or comparison against reference oracles.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
