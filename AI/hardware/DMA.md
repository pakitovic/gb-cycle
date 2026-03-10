# DMA

## Scope

Own OAM DMA behavior now and leave architectural room for CGB HDMA and related access blocking rules later.

## Hardware model

DMA is not an instant copy when accuracy matters. Represent transfer progress and blocking behavior explicitly.

Treat DMA as another bus actor competing for access over time, not as a side effect detached from normal bus ownership rules.

## Responsibilities

- `FF46`-triggered OAM DMA start and source-page latching
- OAM DMA transfer state
- DMA kind selection and future per-kind timing policy
- source-page latch and per-step transfer progress
- DMA-side state and policy inputs for bus blocking and visibility rules during transfer
- future HDMA integration points

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
- Do not model DMA at instruction granularity or as a deferred "consume N cycles at the end" event.

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

## Implementation notes for this repo

- Model transfer progress explicitly.
- Keep DMG OAM DMA and future CGB HDMA conceptually separated.
- Prefer designs where DMA consumes bus activity over time so CPU-visible restrictions arise naturally from arbitration rather than a one-shot special case.
- Keep bus arbitration centralized: DMA should request transfer work, while the bus should expose the resulting blocked-access semantics.
- A scheduler shape where `cpu.tick()`, `dma.tick()`, `ppu.tick()`, `timer.tick()`, and `apu.tick()` all advance on the same T-cycle timeline is the intended baseline, even if orchestration details differ internally.
- Keep `FF46` as the MMIO trigger that configures DMA state; do not bury the whole transfer inside a bus write handler.
- Even if the first implementation only supports `DmaKind::Oam`, structure the subsystem so later `Gdma` and `Hdma` kinds fit without redesigning the contract.
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
