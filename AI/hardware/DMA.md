# DMA

## Scope

Own OAM DMA behavior now and leave architectural room for CGB HDMA and related access blocking rules later.

## Hardware model

DMA is not an instant copy when accuracy matters. Represent transfer progress and blocking behavior explicitly.

## Responsibilities

- OAM DMA transfer state
- bus blocking and visibility rules during transfer
- future HDMA integration points

## Registers / MMIO

- `DMA`
- future CGB HDMA registers

## Timing / accuracy requirements

- Describe when CPU and PPU access is blocked.
- Do not hide DMA behind a one-shot memory copy if the target model requires visible timing.
- Keep the design ready for HDMA without rewriting the API surface later.

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

## Implementation notes for this repo

- Model transfer progress explicitly.
- Keep DMG OAM DMA and future CGB HDMA conceptually separated.

## Known pitfalls

- implementing DMA as an invisible instant copy
- forgetting access restrictions during transfer

## Open questions

- where DMA scheduling should attach to the future scheduler/clock domain
