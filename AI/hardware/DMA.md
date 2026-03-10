# DMA

## Scope

Own OAM DMA behavior now and leave architectural room for CGB HDMA and related access blocking rules later.

## Hardware model

DMA is not an instant copy when accuracy matters. Represent transfer progress and blocking behavior explicitly.

Treat DMA as another bus actor competing for access over time, not as a side effect detached from normal bus ownership rules.

## Responsibilities

- OAM DMA transfer state
- source-page latch and per-step transfer progress
- bus blocking and visibility rules during transfer
- future HDMA integration points

## Registers / MMIO

- `DMA`
- future CGB HDMA registers

## Timing / accuracy requirements

- Describe when CPU and PPU access is blocked.
- Do not hide DMA behind a one-shot memory copy if the target model requires visible timing.
- Keep the design ready for HDMA without rewriting the API surface later.
- DMA progress and blocking should be expressible on the same T-cycle timeline as CPU and PPU activity.
- OAM DMA should be modeled as a `160`-byte transfer whose observable effects unfold over time rather than as a single commit.
- Interactions between DMA source access, CPU-visible blocking, and OAM visibility should remain explicit and testable.

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

## Tests

- Mooneye DMA tests
- focused OAM-blocking tests
- transfer-progress and completion-order tests

## Implementation notes for this repo

- Model transfer progress explicitly.
- Keep DMG OAM DMA and future CGB HDMA conceptually separated.
- Prefer designs where DMA consumes bus activity over time so CPU-visible restrictions arise naturally from arbitration rather than a one-shot special case.
- Keep bus arbitration centralized: DMA should request transfer work, while the bus should expose the resulting blocked-access semantics.
- OAM DMA is the next natural timing deep-dive because it sits at the intersection of CPU accesses, bus arbitration, memory visibility, and PPU/OAM rules.

## Known pitfalls

- implementing DMA as an invisible instant copy
- forgetting access restrictions during transfer

## Open questions

- where DMA scheduling should attach to the future scheduler/clock domain
