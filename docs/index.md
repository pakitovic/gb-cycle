# Documentation Handbook Index

Read the matching file directly; this index is a routing guide plus a summary of document authority boundaries, but detailed behavioral rules still live in the owning file.

## Mandatory external-ROM regression workflow

When working on already-known external ROM failures or rerunning curated ROM suites to evaluate a timing-sensitive change, always capture a baseline copy of the matching persisted report before the work, capture the final report again after the run, and compare the two before deciding whether the change is worth keeping. Use `/test/gb-emulator-shootout/` for promoted GB Emulator Shootout suites, `/test/docboy/` for the large experimental DocBoy single-machine suites, `/test/gbmicrotest/` for the gbmicrotest report suite, and the standalone exploratory report directories such as `/test/blargg/`, `/test/mooneye/`, `/test/little-things-gb/`, `/test/nitro2k01/`, `/test/magen/`, `/test/mealybug-tearoom-tests/`, `/test/samesuite/`, `/test/wilbertpol/`, `/test/rtc3test/`, or `/test/mbc3-tester/`; see [`TESTING.md`](TESTING.md) for the authoritative workflow details.

## Global docs

- [`ARCHITECTURE.md`](ARCHITECTURE.md): project goals, crate layout, subsystem boundaries, and portability rules.
- [`EXECUTION.md`](EXECUTION.md): implementation workflow and change policy.
- [`CODING-RULES.md`](CODING-RULES.md): Rust design rules, API style, and optimization discipline.
- [`REFERENCES.md`](REFERENCES.md): primary documentation, hardware research, executable references, and open-source consultation order.
- [`ROADMAP.md`](ROADMAP.md): index linking to per-phase documents under `roadmap/`.
- [`TODO.md`](TODO.md): active TODO ledger for concrete remaining work across roadmap phases.
- [`TESTING.md`](TESTING.md): unit, integration, ROM-based, differential, determinism, and DMG-closure validation strategy.

## Core design docs

- [`info/MODEL-AXES.md`](info/MODEL-AXES.md): usage and migration guidance for `ConsoleModel`, `OperatingMode`, `HostPlatform`, and `CapabilitySet`.
- [`info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md): accuracy terminology, confidence levels, and timing expectations.
- [`info/INFRARED.md`](info/INFRARED.md): CGB infrared topologies, Pokémon Pikachu 2 accessory behavior, and western GSC Mystery Gift sender behavior.

## Frontend and tooling docs

- [`info/CLI.md`](info/CLI.md): full usage guide for the headless `gb-cli` runner.
- [`info/DESKTOP.md`](info/DESKTOP.md): full usage guide for the SDL3 `gb-desktop` frontend.
- [`info/BENCHMARK.md`](info/BENCHMARK.md): full usage guide for the shared `gb-benchmark` case format, `cargo rom-bench`, and benchmark artifacts.
- [`info/ROM-SUITES.md`](info/ROM-SUITES.md): external ROM suite workflow — fetching, running, differential oracles, and commercial ROM testing.

## Authority map

- [`ARCHITECTURE.md`](ARCHITECTURE.md) owns crate/module layout, subsystem boundaries, and ownership rules.
- [`info/MODEL-AXES.md`](info/MODEL-AXES.md) owns code-facing guidance for choosing between the public model axes and their derived capability view; it does not redefine hardware behavior.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) plus [`info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md) jointly own the project-level global scheduler contract and shared per-T-cycle ordering.
- [`EXECUTION.md`](EXECUTION.md) owns implementation workflow, change-scope discipline, and roadmap-follow-up recording policy.
- [`CODING-RULES.md`](CODING-RULES.md) owns Rust-facing code style, API clarity expectations, and optimization discipline.
- [`REFERENCES.md`](REFERENCES.md) owns the generic source-consultation order and open-source reference tier unless a subsystem handbook overrides it explicitly.
- [`info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md) owns shared timing vocabulary and project-wide temporal constraints.
- [`TESTING.md`](TESTING.md) owns project-wide validation policy and cross-subsystem testing expectations.
- [`ROADMAP.md`](ROADMAP.md) plus `roadmap/*.md` own implementation sequencing and phase context; they do not redefine subsystem behavior.
- [`TODO.md`](TODO.md) owns the active TODO ledger for concrete remaining work across phases.
- `hardware/*.md` own subsystem-specific behavior, MMIO semantics, timing expectations, and subsystem-specific validation detail.
- [`hardware/PPU-REIMPLEMENTATION.md`](hardware/PPU-REIMPLEMENTATION.md) owns repo-local migration constraints, seam-specific guardrails, and regression watch points for PPU rewrites; it does not override [`hardware/PPU.md`](hardware/PPU.md).
- [`hardware/BOOT-ROM.md`](hardware/BOOT-ROM.md) owns startup-path semantics such as real boot, skip boot, `FF50` handoff, and post-boot snapshot policy.

When guidance overlaps, the more specific document wins:

- `hardware/*.md` over generic docs for subsystem behavior
- [`info/MODEL-AXES.md`](info/MODEL-AXES.md) over generic prose when the question is "which public model type should code consult here?", but not over subsystem handbooks for hardware truth
- [`ARCHITECTURE.md`](ARCHITECTURE.md) over [`README.md`](../README.md) or roadmap prose for layout and ownership
- [`info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md) over [`README.md`](../README.md) or roadmap prose for shared timing claims
- [`REFERENCES.md`](REFERENCES.md) over generic prose for consultation order unless a subsystem handbook explicitly refines it
- [`TESTING.md`](TESTING.md) over roadmap prose for generic validation policy
- [`ROADMAP.md`](ROADMAP.md) only for implementation order and remaining work tracking

Subsystem handbooks may refine the generic reference consultation order from [`REFERENCES.md`](REFERENCES.md) when a specific subsystem has a stronger oracle.

The project-wide timing baseline is T-cycle based; see [`ARCHITECTURE.md`](ARCHITECTURE.md), [`info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md), and [`hardware/CPU.md`](hardware/CPU.md). The project-wide CPU baseline is a fine-grained fetch/decode/execute model with explicit bus-visible steps; see [`hardware/CPU.md`](hardware/CPU.md). The project-wide PPU baseline is dot-by-dot with explicit fetcher/FIFO behavior; see [`hardware/PPU.md`](hardware/PPU.md). Repo-local PPU rewrite constraints live in [`hardware/PPU-REIMPLEMENTATION.md`](hardware/PPU-REIMPLEMENTATION.md); [`hardware/PPU.md`](hardware/PPU.md) remains authoritative for hardware behavior. The global scheduler phase contract lives in [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md). The cartridge handbook owns header-driven mapper classification, special-cartridge taxonomy, the cartridge-specific compatibility-category matrix, and cartridge-persistence semantics; [`ARCHITECTURE.md`](ARCHITECTURE.md) owns the central compatibility-policy shape plus the top-level boundary between cartridge persistence and whole-machine save states; [`TESTING.md`](TESTING.md) owns CI/oracle usage of execution modes and save/load determinism policy; see [`hardware/CARTRIDGES-MBC.md`](hardware/CARTRIDGES-MBC.md). Use [`ROADMAP.md`](ROADMAP.md) when a task needs phase context, when resuming incomplete work, or when documenting known remaining gaps after an implementation.

## Hardware docs

- [`hardware/CPU.md`](hardware/CPU.md)
- [`hardware/BUS.md`](hardware/BUS.md)
- [`hardware/MEMORY.md`](hardware/MEMORY.md)
- [`hardware/INTERRUPTS.md`](hardware/INTERRUPTS.md)
- [`hardware/TIMER.md`](hardware/TIMER.md)
- [`hardware/PPU.md`](hardware/PPU.md)
- [`hardware/PPU-REIMPLEMENTATION.md`](hardware/PPU-REIMPLEMENTATION.md)
- [`hardware/DMA.md`](hardware/DMA.md)
- [`hardware/APU.md`](hardware/APU.md)
- [`hardware/JOYPAD.md`](hardware/JOYPAD.md)
- [`hardware/PRINTER.md`](hardware/PRINTER.md)
- [`hardware/LINK.md`](hardware/LINK.md)
- [`hardware/SERIAL.md`](hardware/SERIAL.md)
- [`hardware/CARTRIDGES-MBC.md`](hardware/CARTRIDGES-MBC.md)
- [`hardware/GAME-BOY-CAMERA.md`](hardware/GAME-BOY-CAMERA.md)
- [`hardware/BOOT-ROM.md`](hardware/BOOT-ROM.md)
- [`hardware/CGB.md`](hardware/CGB.md)
- [`hardware/SGB.md`](hardware/SGB.md)

Each hardware file should capture:

- what the subsystem owns
- which registers and timing rules matter
- what must remain explicit in code
- the best primary references
- the best emulator references for comparison
- the most relevant tests and pitfalls
