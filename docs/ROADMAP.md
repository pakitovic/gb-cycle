# DMG ROADMAP — T-Cycle-Based Game Boy DMG Core

Implementation roadmap for the project's **DMG** core: T-cycle based, PPU dot-by-dot, Mode 3 with pixel FIFO, architecturally prepared for CGB.

This is a living document. Update it when phase structure, scope, or done criteria change. Keep TODOs in [TODO.md](TODO.md).

## Scope

DMG only. CGB-specific features (double speed, VRAM/WRAM banking, CGB palettes, HDMA/GDMA) are out of scope. CGB compatibility is considered at the architectural level only.

## Authority boundaries

This roadmap sequences work but is not the behavioral source of truth. For subsystem behavior, layout, timing, and validation policy, follow the owning `docs/` file — see [index.md](index.md) for the full authority map. If roadmap prose drifts from those documents, update the roadmap.

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

## Open work

- [Open TODOs](TODO.md) — active TODO ledger and PPU checkpoint.
- Phase `4` follow-up: the active `Mode 3` refactor now has explicit coverage for the startup post-alignment rule where the first real BG push skips the ordinary entry delay, exported startup-seam snapshots, and dot-level PPU trace fields for current fetch/push/fill/FIFO ownership. Trace-driven follow-up has already closed two narrower `LCDC.3` ownership edges (late `VisibleTile2` push, `VisibleTile3` still in `TileDataHigh`) without regressing the Donkey Kong desktop gate, and a newer sprite-coupled `ly = 10` regression now shows the startup tail renders correctly once panel blanking is lifted. The LCD restart lane also closed both `mooneye ppu/lcdon_write_timing-GS` and `mooneye ppu/lcdon_timing-GS` by separating CPU-visible OAM-write publication from owner bus ownership at the exact `Mode 2 -> 3` boundary and by suppressing CPU-visible `STAT` coincidence on the first dot of a new line after restart. The DMG OAM-corruption trigger path is green again through `blargg oam_bug/4-scanline_timing.gb` and `oam_bug/5-timing_bug.gb` after moving trigger classification back to live `Mode 2` ownership in the PPU instead of a coarse blocked-access flag. Treat the remaining `m3_lcdc_tile_sel_change` debt as separate follow-up, and move the next primary closure target to the sprite-coupled `intr_2_mode0_timing_sprites` seam plus the early-raster `daid/ppu_scanline_bgp.gb` gate.
- Phase `6` follow-up: MBC3 currently keeps a deliberate compatibility deviation for `cpp/latch-rtc-test.gb`: the first RTC latch still requires `0x00 -> 0x01`, but follow-up non-zero writes are also accepted once a valid snapshot exists because instrumentation of that ROM showed repeated non-zero relatch commands without re-arming zeros. Revisit that legacy relatch rule if the curated oracle policy moves back toward the stricter `Pan Docs` model.
- Phase `6` follow-up: MBC3 also keeps `0x04..=0x07` as explicit reserved selectors instead of widening standard SRAM banking to `$00-$07`. Current `Pan Docs` wording says `$00-$07` are RAM-bank selectors, but the retained curated `cpp/rtc-invalid-banks-test.gb` oracle writes and re-reads through `0x04..=0x07` and only stays green when those selector states remain invalid. Revisit the policy if hardware evidence or a stronger oracle closes that source conflict.
- Phase `6` follow-up: MBC3 now records the recommended RTC access-spacing state as `rtc_access_ready_at` on timed RTC-register reads and writes, but the emulator still treats that state as advisory only. `Pan Docs` only recommends the `4 us` spacing without defining an early-access penalty, and a current `SameBoy` cross-check does not expose one either, so keep enforcement deferred until a stronger dedicated oracle or hardware evidence exists.
- Phase `6` follow-up: the cartridge-header parser now preserves `0x013F-0x0142` separately but keeps CGB-era titles conservatively decoded as `15` visible characters. `Pan Docs` documents an additional `11`-character layout when those bytes are really a manufacturer code, but the raw header does not provide a reliable discriminator. Revisit this only if stronger hardware evidence or a clearly scoped per-ROM metadata rule can separate the two layouts without truncating valid `15`-character titles.
- Phase `7` follow-up: CH3 active wave-RAM MMIO semantics remain specified only for the DMG family. Keep the APU architecture ready for a later CGB-specific policy, but do not lock in a fake CGB CH3 active-access contract through tests or docs before the CGB APU lane exists.
- Phase `7` follow-up: CH1 / CH2 / CH3 / CH4 extra-length clocking now has an explicit per-model policy seam, but the current `ConsoleModel` surface still cannot distinguish the documented `CGB-02` exception from later CGB revisions. Keep the generic DMG/later-CGB rule as a conservative fallback and do not claim revision-accurate CGB behavior until a revision-scoped oracle closes that gap.
- Phase `7` follow-up: `SkipBoot` / direct-boot APU startup is only partially synthesized today. The current DMG baseline keeps visible `NRxx`, powered state, wave-RAM startup policy, channel-active reconstruction, and shared-divider-derived `DIV-APU` aligned, but HPF history plus CH1/CH2/CH3/CH4 deeper hidden runtime continuation are still repo-local defaults rather than verified boot-handoff state. Keep that gap explicit in docs and tests until a stronger oracle closes it.
- Phase `7` follow-up: CH1 / CH2 / CH4 now model the only zombie-mode path that `Pan Docs` calls useful and cross-unit-consistent, namely live `NRx2` writes in increase mode with pace `0` incrementing the current volume modulo `16`. The rest of the zombie-mode write matrix still varies by hardware revision, so keep any stronger DMG claims deferred until a revision-scoped oracle or hardware-backed policy closes that gap.
- Phase `7` follow-up: the DMG baseline now follows Pan Docs for the "all DACs off" output disconnect, but it still approximates the per-channel DAC-off analog path as an immediate step to `0` instead of the documented model-dependent fade. Keep that residual gap explicit until a stronger oracle or hardware-backed fade policy closes it.

## Final notes

- This document defines the recommended implementation order, not necessarily the exact merge order if work happens in parallel.
- Whenever a later block requires additional observability, the `debugger/` infrastructure should be expanded incrementally without changing its transversal role.
- Any local simplification that contradicts the T-cycle model or the dot-by-dot PPU must be treated as explicit and documented technical debt.
- If a conflict appears between ease of implementation and temporal fidelity, this roadmap prioritizes temporal fidelity as long as the design remains maintainable.
