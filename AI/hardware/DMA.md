# DMA

## Scope

Own OAM DMA behavior now and leave architectural room for CGB HDMA and related access blocking rules later.

## Hardware model

DMA is not an instant copy when accuracy matters. Represent transfer progress and blocking behavior explicitly.

Treat DMA as another bus actor competing for access over time, not as a side effect detached from normal bus ownership rules.

## Responsibilities

- common DMA-controller contract and active-transfer lifecycle
- `FF46`-triggered OAM DMA start and source-page latching
- OAM DMA transfer state
- DMA kind selection and future per-kind timing policy
- source-page latch and per-step transfer progress
- DMA-side state and policy inputs for bus blocking and visibility rules during transfer
- future HDMA integration points

## Common transfer-architecture baseline

The DMA subsystem should already expose one T-cycle-based transfer model even while the current functional target is only DMG OAM DMA.

The abstraction must keep these dimensions explicit:

- source
- destination
- total remaining work
- operational block size or unit size
- timing policy
- CPU impact policy
- memory-region impact
- advance condition
- lifecycle state

Do not flatten DMA into a generic `memcpy_async(src, dst, len)` helper. OAM DMA, future CGB GDMA, and future CGB HDMA differ in bus ownership, CPU visibility, transfer granularity, and stop/resume rules even when all of them ultimately copy bytes.

## Transfer-family baseline

- Support two conceptual DMA families from the start:
  - full-burst transfers with one explicit in-flight duration on the shared T-cycle timeline
  - windowed or block transfers that only advance in eligible windows such as HBlank
- The shared API must distinguish a fixed total-duration transfer from one that is composed of repeated periodic blocks.
- Current DMG OAM DMA is a full-burst transfer.
- Future CGB GDMA should fit the same full-burst family without redesigning the scheduler.
- Future CGB HDMA should fit the windowed or block-transfer family without redesigning the scheduler.

## Transfer-state and lifecycle baseline

- A shape such as `DmaController` owning one or more typed `ActiveTransfer` values is the intended design direction.
- Each active transfer should expose at least:
  - `kind`
  - `source`
  - `destination`
  - `remaining_bytes`
  - `block_size`, such as future `0x10`-byte HDMA-style blocks
  - `timing_policy`
  - `cpu_impact_policy`
  - `memory_region_impact`
  - `advance_condition`
  - `is_active`
- Lifecycle should be explicit rather than inferred from `remaining_bytes == 0`.
- The common lifecycle should be able to represent `Idle`, `Starting`, `Active`, `Completed`, and future `Cancelled`.
- The subsystem should expose "this transfer affects arbitration or CPU visibility on this T-cycle" separately from "this transfer commits a byte or block on this T-cycle."

## CPU impact policy baseline

- The common contract should already support at least these CPU-impact policies:
  - `NoCpuStallButBusRestriction`
  - `CpuFullyStalledUntilDone`
  - `CpuStalledPerBlock`
- Current DMG OAM DMA maps to `NoCpuStallButBusRestriction`: the CPU keeps executing on the shared T-cycle timeline, but on DMG it only retains normal HRAM access while the transfer is active.
- Future CGB GDMA should be modelable as `CpuFullyStalledUntilDone`.
- Future CGB HDMA should be modelable as `CpuStalledPerBlock`.
- The CPU must not hard-code these policies locally; it should observe the result of central arbitration plus DMA-published state.

## Source and destination contract baseline

- DMA-owned register handling should validate and normalize transfer endpoints inside the DMA subsystem rather than leaving that work to bus call sites.
- OAM DMA should continue to validate the latched source page so the effective source range remains `XX00-XX9F` with `XX` in the documented `00-DF` range.
- The common transfer contract should already leave room for future aligned or forced endpoints, such as CGB VRAM DMA source and destination alignment to `0x10` and a destination forced into VRAM.
- Source and destination restrictions belong to the transfer kind and its controller state, not to frontend register wrappers.

## Registers / MMIO

- `FF46` / `DMA`
- future CGB HDMA registers

## DMG OAM DMA baseline

- Writing `FF46` starts OAM DMA by latching the written high byte as the source page.
- The source range is `XX00-XX9F`, where `XX` is the latched value from `FF46`.
- The destination range is always `FE00-FE9F`.
- A first correct implementation should explicitly track at least `active`, `kind`, `source_high`, `source_addr_current`, `dest_addr_current`, `bytes_remaining`, and `elapsed_dots` or an equivalent byte-phase timing state.
- `FF46` must arm and configure the DMA subsystem; it must not perform the `160`-byte copy immediately as a side effect of the register write.

## `FF46` MMIO contract baseline

- Treat `FF46` as a write-triggered DMA control register with immediate side effects, not as a passive byte that another subsystem polls later.
- The authoritative action of a write to `FF46` is "start OAM DMA with this source page" on that access, not "update a memory-mapped variable that may later cause DMA."
- Any MMIO-visible `FF46` readback should come from DMA-owned latched state rather than from a generic bus byte.
- Internal debug state for DMA may exist separately, but it must not replace explicit in-flight transfer state or the MMIO-owned register view.

## Timing / accuracy requirements

- Describe when CPU and PPU access is blocked.
- Do not hide DMA behind a one-shot memory copy if the target model requires visible timing.
- Keep the design ready for HDMA without rewriting the API surface later.
- DMA progress and blocking should be expressible on the same T-cycle timeline as CPU and PPU activity.
- OAM DMA should be modeled as a `160`-byte transfer whose observable effects unfold over time rather than as a single commit.
- For the current DMG-family target, OAM DMA lasts `160` M-cycles = `640` dots at normal speed.
- Treat those `640` dots as the current hard requirement for DMG-family work; future CGB speed differences should extend the model later rather than weaken the DMG baseline.
- On the shared T-cycle timeline, DMG OAM DMA should conceptually progress at `1` byte every `4` dots.
- Interactions between DMA source access, CPU-visible blocking, and OAM visibility should remain explicit and testable.
- CPU execution should continue during OAM DMA, but on DMG the CPU should only retain normal HRAM access while the transfer is active.
- DMA destination writes into OAM must still flow through the same central access-arbitration model used elsewhere; do not create a magical OAM backdoor.
- On the scheduler timeline, DMA progress should be advanced before current-cycle bus arbitration and CPU access decisions so the bus sees the live DMA state for that same T-cycle.
- DMA owns transfer progress and source/destination stepping; the bus owns the resulting blocked-access policy observed by CPU and other requesters.
- Do not model DMA at instruction granularity or as a deferred "consume N cycles at the end" event.

## Scheduler integration baseline

- DMA transfers must tick from the global scheduler, not from the CPU execution path.
- On the project's shared T-cycle scheduler, DMA belongs to the autonomous-peripheral portion of the cycle rather than to CPU micro-ops.
- The DMA subsystem should publish current-cycle bus constraints before the bus arbitrates the CPU access for that same T-cycle.
- The DMA subsystem should expose at least:
  - whether CPU access is currently impacted
  - which memory region is currently impacted, such as `Oam`, `Vram`, or no special region
  - whether the transfer performs source or destination work on this T-cycle
- The bus should consult DMA-owned state, not `FF46` or future `HDMA1-5` register contents, when deciding live arbitration behavior.
- A split such as `dma_controller.tick(ctx)`, `dma_controller.bus_constraints(ctx)`, and `dma_controller.commit_memory_writes(ctx)` is a good fit for keeping transfer progress, arbitration publication, and writeback explicit on the T-cycle timeline.

## OAM DMA as the first concrete transfer

- OAM DMA should be implemented as one concrete transfer kind inside the common DMA controller, not as a one-off path outside the subsystem contract.
- That transfer instance should fix these DMG-family properties:
  - fixed size of `160` bytes
  - fixed destination range `FE00-FE9F`
  - linear source and destination stepping
  - fixed DMG duration of `640` T-cycles at normal speed
  - CPU impact policy `NoCpuStallButBusRestriction`
  - memory-region impact `Oam`
- OAM DMA remains the first closed functional milestone, but the subsystem contract must already leave room for later block-based and HBlank-conditioned transfers.
- Visual consequences of OAM DMA during Mode `2` or `3` remain owned by the PPU or OAM-side logic; DMA owns transfer state, duration, and region-impact publication.

## Deferred but required extension seams

- This document still keeps the current functional scope to DMG OAM DMA.
- Do not implement `FF51-FF55`, real GDMA, real HDMA, CGB VRAM or WRAM banking semantics, or double-speed-specific DMA timing as part of the DMG milestone.
- Even so, the common DMA contract should already permit:
  - future block-based transfers
  - future `0x10`-byte HDMA-style blocks
  - future HBlank-gated advance conditions
  - future cancellation and status visibility
  - future transfer kinds whose advance depends on global state such as CPU `HALT` or `STOP`

## Dependencies

- bus
- PPU
- memory/MMIO map
- model/revision configuration

## Primary references

- Pan Docs DMA sections
- AntonioND timing material
- relevant CGB documentation for HDMA

## Open-source emulator references

Priority order:

1. SameBoy
2. binjgb
3. accurateboy
4. Mooneye GB
5. Danger Boy
6. Gambatte

## Tests

- Mooneye DMA tests
- `FF46` trigger and source-page selection tests
- focused OAM-blocking tests
- DMG total-duration tests for `640` dots / `160` M-cycles
- HRAM-only CPU access tests during active DMG OAM DMA
- transfer-progress and completion-order tests
- tests that DMA-visible blocking for a T-cycle matches the DMA state produced by that same cycle's scheduler step
- lifecycle-visibility tests covering `Idle`, active start, completion, and future-compatible cancellation hooks
- tests that bus-constraint publication is observable separately from actual byte-copy work on a T-cycle
- tests for common DMA-controller state such as `kind`, `remaining_bytes`, and current CPU-impact policy
- unit tests for a simulated `0x10`-byte block or windowed transfer shape that is not yet wired to CGB MMIO

## Implementation notes for this repo

- Model transfer progress explicitly.
- Keep DMG OAM DMA and future CGB HDMA conceptually separated.
- Prefer designs where DMA consumes bus activity over time so CPU-visible restrictions arise naturally from arbitration rather than a one-shot special case.
- Keep bus arbitration centralized: DMA should request transfer work, while the bus should expose the resulting blocked-access semantics.
- A scheduler shape where `cpu.tick()`, `dma.tick()`, `ppu.tick()`, `timer.tick()`, and `apu.tick()` all advance on the same T-cycle timeline is the intended baseline, even if orchestration details differ internally.
- Keep `FF46` as the MMIO trigger that configures DMA state; do not bury the whole transfer inside a bus write handler.
- Even if the first implementation only supports `DmaKind::Oam`, structure the subsystem so later `Gdma` and `Hdma` kinds fit without redesigning the contract.
- Keep transfer timing, CPU-impact policy, advance condition, and address validation as DMA-owned concepts rather than scattering them through bus or CPU code.
- Let the bus consume a published DMA constraint view instead of branching on `if dma_kind == ...` across several call sites.
- OAM DMA is the next natural timing deep-dive because it sits at the intersection of CPU accesses, bus arbitration, memory visibility, and PPU/OAM rules.

## First DMG OAM DMA milestone

- add a dedicated DMA subsystem with explicit inactive/active transfer state and `FF46` start handling
- integrate `dma.tick()` into the shared T-cycle scheduler
- enforce DMG CPU HRAM-only access semantics while OAM DMA is active
- route DMA reads and OAM writes through the same central arbitration model used by the rest of the system
- add focused tests for total duration, correct `160`-byte copy, CPU blocking, HRAM accessibility, and LCD-enabled interaction

## Known pitfalls

- implementing DMA as an invisible instant copy
- suspending the CPU completely instead of modeling its restricted bus access during DMA
- forgetting access restrictions during transfer
- routing OAM DMA through a special path that bypasses bus arbitration
- letting `FF46` perform the full transfer immediately instead of only arming DMA state
- hard-coding the subsystem as a one-off DMG `FF46` copy path that cannot grow into CGB DMA variants

## Open questions

- where DMA scheduling should attach to the future scheduler/clock domain
