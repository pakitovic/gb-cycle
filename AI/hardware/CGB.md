# CGB

## Scope

Own Color Game Boy-specific behavior: double speed, VRAM banks, WRAM banks, palettes, HDMA, and other model-specific extensions beyond shared DMG behavior.

## Hardware model

Design interfaces today that do not block CGB tomorrow. Separate DMG-only, shared, and CGB-only behavior explicitly.

## Responsibilities

- double-speed behavior
- banked VRAM and WRAM behavior
- color palettes
- CGB-only DMA/HDMA features
- model capability flags and feature gates

## Registers / MMIO

- CGB palette registers
- VRAM/WRAM bank registers
- speed switch control
- HDMA registers

## Timing / accuracy requirements

- Avoid DMG shortcuts that would break banks, palettes, HDMA, or double speed.
- Keep CGB timing and shared timing differences visible.

## Dependencies

- PPU
- DMA
- bus and memory
- model/revision configuration

## Primary references

- Pan Docs CGB sections
- Gekkio references
- model-specific hardware research

## Open-source emulator references

Priority order:

1. SameBoy
2. GameRoy
3. binjgb
4. Gambatte
5. Mooneye GB

## Tests

- cgb-acid2
- CGB Mooneye tests
- palette/banking/HDMA focused tests

## Implementation notes for this repo

- Model capabilities should be centralized, not spread as random conditionals.
- Shared subsystems should expose clean extension points for CGB-only behavior.

## Known pitfalls

- coupling DMG assumptions into shared APIs
- hiding double-speed effects behind generic timing helpers

## Open questions

- which shared abstractions can remain stable across DMG and CGB without losing clarity
