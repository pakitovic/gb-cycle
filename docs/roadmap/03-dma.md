# Phase 3 — Base DMA

14. **OAM DMA**

#### Goal

Integrate DMG OAM DMA as a real transfer mechanism inside the system architecture through a reusable DMA-controller foundation, coordinated with the scheduler and bus and already prepared for future CGB transfer families without implementing them yet.

#### Modules involved

- `dma/`
- `bus/`
- `scheduler/`
- `cpu/`
- `debugger/`

#### Deliverables

- common `DmaController`-style infrastructure with explicit active-transfer state
- writing to the DMA register triggers the transfer
- OAM DMA as the first concrete transfer kind inside that common infrastructure
- real temporal copy progression on the shared T-cycle timeline
- integration with bus arbitration
- separation between transfer progression and DMA-published arbitration state
- explicit lifecycle and status visibility for active transfers
- observability of DMA start, progress, and completion
- scheduler-visible DMA state that bus arbitration can use on the same T-cycle
- transfer fields that already leave room for block or windowed progression without wiring CGB MMIO yet

#### Done criteria

- the transfer is not implemented as an instantaneous memory copy
- OAM DMA is implemented as an instance of the common DMA transfer infrastructure rather than a one-off path outside it
- arbitration correctly reflects DMA effects on concurrent accesses
- CPU-versus-DMA precedence is decided centrally through bus arbitration instead of CPU-local blocking logic
- DMG OAM DMA still leaves the CPU with HRAM access only while active
- DMA lifecycle and active-state visibility are explicit and traceable
- the infrastructure can already represent future block or windowed transfers without requiring a later scheduler redesign, even though GDMA and HDMA remain out of scope here
- the system can trace DMA over time

Status note (`2026-03-19`): the current repo closes `Phase 3.5` and therefore closes `Phase 3` as a whole. The DMA subsystem now exposes one common transfer contract with explicit lifecycle/status queries independent of `FF46` readback, published bus-impact state, and future-family hooks such as `block_size`, `transfer_family`, and `advance_condition`. OAM DMA runs on the shared T-cycle timeline as a real `160`-byte bus-routed transfer, and traces carry start/progress/completion plus the published DMA bus-impact metadata from the same cycle.

Timing refinement note (`2026-03-19`): source-analysis cross-check against SameBoy commit `208ba4afabffab9edde416f2dbb8ae459e34adb8` (`Core/memory.c`, `GB_IO_DMA` setup, `GB_dma_run`, and `is_addr_in_dma_use`) is now reflected in the repo's DMG OAM-DMA model. The transfer keeps the `640`-T-cycle DMG burst duration, but exposes an explicit `2`-T-cycle start-up seam before the first byte commit. The first byte becomes visible at elapsed T-cycle `2`, later bytes continue every `4` T-cycles, the last byte lands at elapsed T-cycle `638`, and the final `Completed` transition remains visible after the remaining `2`-T-cycle tail.

Oracle-validation note (`2026-03-19`): the same SameBoy cross-check also shows that CPU-side non-HRAM conflict handling is not published during the internal warm-up markers. The repo therefore now keeps the DMA transfer `in flight` through the start-up seam while leaving the published CPU bus state `Unrestricted` until that seam ends, and only then switches to the DMG source-bus-specific restriction. This keeps bus-impact onset explicit in the DMA timing contract instead of inferring it from "transfer armed" alone.

#### Recommended sequencing inside Phase 3

Phase `3` should be executed as narrow subphases. No subphase counts as closed unless its local acceptance criteria land together with focused automated coverage and move the phase-level DMA/bus/scheduler contract forward without reintroducing CPU-local blocking logic, instant-copy shortcuts, or DMA-owned bus decode.

1. `Phase 3.1` - Common DMA transfer contract and `FF46`-owned OAM descriptor.
   Acceptance criteria: the DMA subsystem replaces the ad hoc `OamStartRequested`-style placeholder with one typed active-transfer shape, `FF46` writes still latch the visible source page immediately, OAM DMA start normalization derives the effective `XX00-XX9F` source range plus fixed `FE00-FE9F` destination inside DMA-owned code, and the transfer record already carries explicit DMG properties such as total length, timing policy, CPU-impact policy, and memory-region impact. Validation gate: focused unit tests cover `FF46` readback, source-page normalization, fixed OAM destination, `160`-byte length, DMG timing-policy metadata, CPU-impact metadata, and direct-boot startup state without yet requiring whole-machine copy progression.
2. `Phase 3.2` - Scheduler-driven DMA timeline and current-cycle state publication.
   Acceptance criteria: DMA gains an autonomous-peripheral `tick` path on the shared scheduler timeline, transfer progression becomes explicit per T-cycle rather than implicit in one later bulk copy, `Starting -> Active -> Completed` becomes observable on that timeline, current-cycle DMA state is published before bus arbitration for the same T-cycle, and DMG OAM DMA duration is modeled as `640` dots with byte-phase visibility. Validation gate: focused unit and integration tests cover state progression across `Idle`, `Starting`, `Active`, and `Completed`, `1` byte every `4` dots progression metadata, `640`-dot total duration, deterministic stepping, and trace visibility for start and completion points.
3. `Phase 3.3` - Central arbitration closure and DMG source-bus-aware CPU behavior.
   Acceptance criteria: the bus consumes one common DMA constraint view instead of peeking at `FF46` or transfer internals, CPU-versus-DMA precedence stays centralized in arbitration rather than in CPU-local special cases, live DMG OAM DMA publishes source-bus-aware CPU blocking while active, and the PPU can consume one common OAM-impact signal rather than transfer-specific register knowledge. Validation gate: focused arbitration tests cover CPU access during active DMA for both external-bus and video-bus source shapes, blocked-read and ignored-write behavior on the conflicted bus family, unrestricted DMA requester access through the same arbitration path, DMA precedence over ordinary PPU region-policy checks, and same-cycle coherence between published DMA state and the bus decision the CPU observes.
4. `Phase 3.4` - Real OAM data movement through the shared bus model.
   Acceptance criteria: DMA source reads and OAM destination writes happen through the same central bus/arbitration model used by the rest of the machine, OAM DMA copies the full `160` bytes from the latched source page to OAM over time instead of by side effect, transfer-progress state and copied bytes remain separately observable on the timeline, and completion clears the in-flight transfer state without bypassing lifecycle visibility. Validation gate: integration tests cover source-page selection, correct `160`-byte copy contents, partial-progress snapshots before completion, OAM contents after completion, and completion ordering relative to the last transfer T-cycle.
5. `Phase 3.5` - Future transfer-family hooks, observability, and phase closure.
   Acceptance criteria: the common DMA API exposes lifecycle and status queries without depending on one origin register, the transfer contract already carries fields such as `block_size` and `advance_condition` for future block/windowed DMA families, traces expose DMA start/progress/completion plus published bus-impact state, and the phase closes with explicit TODOs only if a concrete remaining gap still blocks full Phase `3` done criteria. Validation gate: focused tests cover lifecycle/status visibility, current bus-impact publication, and at least one simulated `0x10`-byte block-style transfer shape that is not yet wired to real CGB MMIO; before closing the phase, the resulting DMA ordering is cross-checked against SameBoy at the source-analysis level when a timing-sensitive question remains.

#### Subphase exit rule

Every Phase `3` subphase should end with:

- targeted unit and integration coverage for the newly closed DMA contract
- updated traces or snapshots when observable DMA ordering changes
- `cargo test -q` passing locally at minimum, and `make ci` when the subphase changes repo tooling or shared workflow-critical infrastructure
- a roadmap TODO recorded immediately if the subphase ships with a concrete uncovered gap

#### Risks if delayed too much

- the need to rewrite the bus
- problematic integration with sprites and OAM
- false positives in CPU or PPU behavior because real access conflicts were missing

