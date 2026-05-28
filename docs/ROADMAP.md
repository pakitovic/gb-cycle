# gb-cycle Roadmap

This document is the phase index for the shared T-cycle Game Boy core: DMG closure, CGB expansion, and SGB/SGB2 host-shell support. It owns implementation order and phase-level context only; subsystem behavior, validation policy, and operational ROM-suite details live in their owning docs.

## Authority and maintenance

Use this file to choose the relevant phase document before changing implementation scope. If a task changes behavior, update the owning hardware or policy document as well; if a task leaves concrete follow-up work, update [`TODO.md`](TODO.md) in roadmap order.

Authority routing:

- [`index.md`](index.md) — full documentation map and conflict-resolution order.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — crate layout, ownership boundaries, scheduler contract, persistence boundaries, and portability rules.
- [`TESTING.md`](TESTING.md) — project-wide validation policy and closure evidence rules.
- [`info/ROM-SUITES.md`](info/ROM-SUITES.md) — external ROM fetching, suite execution, report layout, commercial local manifests, and environment variables.
- [`info/MODEL-AXES.md`](info/MODEL-AXES.md) and [`info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md) — public model axes and shared timing vocabulary.
- `hardware/*.md` — hardware behavior, MMIO semantics, timing details, and subsystem-specific validation checklists.

## Scope summary

Phases 0-9 close the DMG-family core on the shared T-cycle scheduler. Phase 10 extends that same core for CGB features such as double speed, VRAM/WRAM banking, palettes, HDMA/GDMA, CGB audio lanes, RTC behavior, and CGB ROM-suite gates while preserving the accepted DMG baseline. Phase 11 adds SGB/SGB2 as host-shell profiles around the shared GB core, with SNES/SFC-side execution and host-audio details intentionally isolated behind explicit SGB host seams.

Current status:

- Practical DMG Phase 9 closure is accepted; CGB and SGB/SGB2 work build on that baseline instead of redefining it.
- CGB work is tracked under Phase 10 and must not redefine the DMG closure signal.
- SGB/SGB2 work is tracked under Phase 11; current public scope covers host-shell behavior already documented there, while deeper host audio and SNES/SFC execution remain deferred slices.

## Cross-cutting workstreams

- **Scheduler foundation** — Phase 0 architecture, Phase 1 bus/arbitration, Phase 2 IRQ and CPU timing, and regression tests that preserve one deterministic T-cycle timeline.
- **Persistence and replay** — Phase 6 cartridge persistence, Phase 8 whole-machine save states and rewind primitives, and Phase 9 determinism/save-load closure evidence.
- **Validation and reports** — Phase 9 strict DMG evidence, Phase 10 CGB promotion lanes, Phase 11 linked/SGB fixtures, and the operational suite/report policy in [`info/ROM-SUITES.md`](info/ROM-SUITES.md).
- **Model growth** — DMG-family compatibility, CGB operating modes, SGB/SGB2 host profiles, and future revision-specific work routed through [`info/MODEL-AXES.md`](info/MODEL-AXES.md).

## Phase index

| Phase | Focus | Document |
| --- | --- | --- |
| 0 | Verification, debugging, and base architecture infrastructure | [`roadmap/00-infrastructure.md`](roadmap/00-infrastructure.md) |
| 1 | Temporal foundation and hardware access | [`roadmap/01-timing.md`](roadmap/01-timing.md) |
| 2 | CPU and real temporal control | [`roadmap/02-cpu.md`](roadmap/02-cpu.md) |
| 3 | Base DMA | [`roadmap/03-dma.md`](roadmap/03-dma.md) |
| 4 | Base PPU and visible pipeline | [`roadmap/04-ppu.md`](roadmap/04-ppu.md) |
| 5 | Input and simple peripherals | [`roadmap/05-input.md`](roadmap/05-input.md) |
| 6 | Banked cartridges, special cartridges, and cartridge persistence | [`roadmap/06-cartridges.md`](roadmap/06-cartridges.md) |
| 7 | Audio | [`roadmap/07-audio.md`](roadmap/07-audio.md) |
| 8 | Full emulator save states and global serialization strategy | [`roadmap/08-save-states.md`](roadmap/08-save-states.md) |
| 9 | Final DMG hardening, differential validation, and closure | [`roadmap/09-hardening.md`](roadmap/09-hardening.md) |
| 10 | CGB implementation roadmap | [`roadmap/10-cgb.md`](roadmap/10-cgb.md) |
| 11 | SGB/SGB2 implementation roadmap | [`roadmap/11-sgb.md`](roadmap/11-sgb.md) |
