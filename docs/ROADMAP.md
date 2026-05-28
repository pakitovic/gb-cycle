# gb-cycle Roadmap — T-Cycle-Based Game Boy Core

Implementation roadmap for the project's **DMG** core closure, **CGB** expansion, and **SGB/SGB2** host-shell support: T-cycle based, PPU dot-by-dot, Mode 3 with pixel FIFO, and architecturally prepared for model-specific hardware behavior.

This is a living document. Update it when phase structure, scope, or done criteria change. Keep TODOs in [TODO.md](TODO.md).

## Scope

Phases 0 through 9 close the DMG core. CGB-specific features such as double speed, VRAM/WRAM banking, CGB palettes, and HDMA/GDMA stay out of those DMG closure phases except where architectural seams are required. Phase 10 starts the functional CGB implementation roadmap and must preserve the DMG `167/167` ROM gate after every CGB slice. Phase 11 implements SGB/SGB2 as a DMG-compatible GB core plus pluggable SGB/SNES host shell; slices 0-6 are the current public SGB milestone, while slices 7-9 are explicitly deferred to a later SGB host-shell milestone for startup firmware presentation, special audio, and SNES-side execution.

## Authority boundaries

This roadmap sequences work but is not the behavioral source of truth. It defines the recommended implementation order and phase dependencies, not necessarily the exact merge order when work happens in parallel. For subsystem behavior, layout, timing, and validation policy, follow the owning `docs/` file — see [index.md](index.md) for the full authority map. If roadmap prose drifts from those documents, update the roadmap.

## Cross-cutting workstreams

Two workstreams span multiple phases:

- **Scheduler foundation** — explicit scheduler phases (Phase 0), central bus arbitration (Phase 1), IRQ aggregation (Phase 2), cycle logging (Phase 0+), and global-order regression tests (Phase 2+).
- **State persistence** — cartridge persistence boundary (Phase 6), whole-machine save states (Phase 8), and determinism/closure integration (Phase 9). See [`ARCHITECTURE.md`](ARCHITECTURE.md) for the top-level boundary and [`hardware/CARTRIDGES-MBC.md`](hardware/CARTRIDGES-MBC.md) for cartridge-persistence semantics.

## Phases

- [Phase 0 — Verification, debugging, and base architecture infrastructure](roadmap/00-infrastructure.md)
- [Phase 1 — Temporal foundation and hardware access](roadmap/01-timing.md)
- [Phase 2 — CPU and real temporal control](roadmap/02-cpu.md)
- [Phase 3 — Base DMA](roadmap/03-dma.md)
- [Phase 4 — Base PPU and visible pipeline](roadmap/04-ppu.md)
- [Phase 5 — Input and simple peripherals](roadmap/05-input.md)
- [Phase 6 — Banked cartridges, special cartridges, and cartridge persistence](roadmap/06-cartridges.md)
- [Phase 7 — Audio](roadmap/07-audio.md)
- [Phase 8 — Full emulator save states and global serialization strategy](roadmap/08-save-states.md)
- [Phase 9 — Final DMG hardening, differential validation, and closure](roadmap/09-hardening.md)
- [Phase 10 — CGB implementation roadmap](roadmap/10-cgb.md)
- [Phase 11 — SGB/SGB2 implementation roadmap](roadmap/11-sgb.md)
