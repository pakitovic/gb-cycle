# PPU

## Scope

Own LCD/PPU mode progression, rendering state, VRAM/OAM access rules, STAT behavior, fetcher/FIFO behavior, and frame output generation.

## Hardware model

Model PPU modes explicitly. Separate fetcher/FIFO logic when it improves clarity and timing fidelity.

## Responsibilities

- mode transitions and scanline progression
- background, window, and sprite fetch behavior
- pixel priority rules
- STAT/LY/LYC and LCD-visible interrupts

## Registers / MMIO

- `LCDC`
- `STAT`
- `SCX`, `SCY`
- `LY`, `LYC`
- `WX`, `WY`
- `BGP`, `OBP0`, `OBP1`
- OAM and VRAM ownership rules

## Timing / accuracy requirements

- Make mode timing explicit.
- Handle VRAM/OAM locking precisely.
- Explain sprite, window, and FIFO quirks where accuracy depends on them.

## Dependencies

- bus and memory
- interrupt controller
- DMA
- model/revision configuration

## Primary references

- Pan Docs PPU/LCD sections
- AntonioND timing material
- Gekkio and Matt Currie test ROMs where relevant

## Open-source emulator references

Priority order:

1. SameBoy
2. accurateboy
3. binjgb
4. Mooneye GB
5. Danger Boy

## Tests

- dmg-acid2 / cgb-acid2
- mealybug-tearoom-tests
- Mooneye LCD/STAT tests

## Implementation notes for this repo

- Keep mode state explicit.
- Separate rendering backend concerns from internal PPU state.

## Known pitfalls

- STAT interrupt edge handling
- window trigger behavior
- sprite priority and timing shortcuts
- modeling DMA/PPU access conflicts too loosely

## Open questions

- how soon a full fetcher/FIFO model is required for target accuracy
