# DMG ROADMAP — T-Cycle-Based Game Boy DMG Core

Implementation roadmap for the project's **DMG** core: T-cycle based, PPU dot-by-dot, Mode 3 with pixel FIFO, architecturally prepared for CGB.

This is a living document. Update it when phase structure, scope, or done criteria change. Keep TODOs in [TODO.md](TODO.md).

## Scope

DMG only. CGB-specific features (double speed, VRAM/WRAM banking, CGB palettes, HDMA/GDMA) are out of scope. CGB compatibility is considered at the architectural level only.

## Authority boundaries

This roadmap sequences work but is not the behavioral source of truth. It defines the recommended implementation order and phase dependencies, not necessarily the exact merge order when work happens in parallel. For subsystem behavior, layout, timing, and validation policy, follow the owning `docs/` file — see [index.md](index.md) for the full authority map. If roadmap prose drifts from those documents, update the roadmap.

## Cross-cutting workstreams

Two workstreams span multiple phases:

- **Scheduler foundation** — explicit scheduler phases (Phase 0), central bus arbitration (Phase 1), IRQ aggregation (Phase 2), cycle logging (Phase 0+), and global-order regression tests (Phase 2+).
- **State persistence** — cartridge persistence boundary (Phase 6), whole-machine save states (Phase 8), and determinism/closure integration (Phase 9). See `ARCHITECTURE.md` for the top-level boundary and `hardware/CARTRIDGES-MBC.md` for cartridge-persistence semantics.

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
