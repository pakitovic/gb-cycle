# PPU

## Scope

Own LCD/PPU mode progression, rendering state, VRAM/OAM access rules, STAT behavior, fetcher/FIFO behavior, and frame output generation.

## Hardware model

Model PPU modes explicitly. Separate fetcher/FIFO logic when it improves clarity and timing fidelity.

Even in DMG-only work, avoid hard-wiring the design to a single permanent VRAM interpretation or a renderer that only understands four grayscale outputs.
For this project, the PPU should be modeled dot-by-dot, where `1 dot = 1 T-cycle`.

## Responsibilities

- mode transitions and scanline progression
- background, window, and sprite fetch behavior
- pixel priority rules
- STAT/LY/LYC and LCD-visible interrupts
- tile fetcher state
- background and object FIFO state
- pixel FIFO state and output timing

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
- Use a timing base that is compatible with dot-level reasoning so future CGB double-speed support does not require a new temporal model.
- Model scanline timing in dots: `456` dots per scanline and `154` scanlines per frame.
- Treat the full frame as `70224` dots.
- Treat Mode 2 as `80` dots and keep Mode 3 variable by construction instead of forcing a fixed duration.
- Treat Mode 3 as a variable phase in the `172-289` dot range for DMG-family behavior, depending on fetcher/FIFO work and stalls.
- Treat Mode 0 / HBlank as the remainder of the scanline budget after Mode 2 and variable Mode 3 work have completed.
- Treat Mode 1 / VBlank as part of the same real LCD mode schedule, with transitions aligned to scanline and STAT timing rather than as a separate high-level event.
- Output and pipeline progress should be expressible dot-by-dot on the shared T-cycle timeline.
- LCD-visible pixel output should advance at one pixel per dot once the pipeline is producing pixels.
- Treat the minimum Mode 3 length as larger than `160` visible pixels because pipeline startup work is part of the real hardware schedule.
- During Mode 2, OAM scan should progress on a fixed `80`-dot budget while building the per-scanline sprite candidate list for Mode 3.
- Model OAM scan as an ordered traversal of the `40` OAM entries, selecting at most `10` sprites for the current scanline.
- On DMG, CPU OAM access should be treated as blocked during Modes `2` and `3`, while CPU VRAM access should be treated as blocked during Mode `3`.
- When those CPU accesses are blocked, writes should be ignored and reads should return the blocked-access result rather than the underlying stored byte.

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
6. Gambatte

## Tests

- dmg-acid2 / cgb-acid2
- mealybug-tearoom-tests
- Mooneye LCD/STAT tests
- tests for variable Mode 3 timing, SCX discard behavior, and sprite-induced stalls when available
- direct-boot continuity tests that verify the first LCD-visible dots after `SkipBoot` are coherent with the published post-boot `LCDC`, `STAT`, and `LY` snapshot

## Implementation notes for this repo

- Keep mode state explicit.
- Separate rendering backend concerns from internal PPU state.
- Do not implement the PPU as a scanline renderer or a mode-only renderer if the goal is cycle accuracy.
- Mode 2 should be an explicit PPU state with its own dot counter, OAM traversal progress, and temporary visible-sprite list.
- Preserve OAM discovery order during Mode 2 instead of rebuilding an idealized sprite list later.
- Mode 3 should begin by clearing the relevant BG/object FIFO state and resetting the pixel pipeline state instead of assuming pixels are already queued.
- Represent the pixel pipeline explicitly as fetcher plus FIFO, with state advanced one dot at a time.
- Treat background and object pixel queues as explicit hardware-facing concepts; do not collapse all pixel production into a single opaque renderer step.
- A fetcher model with explicit stages such as tile index, tile data low, tile data high, and FIFO push is preferred over opaque bulk tile reads.
- Treat the framebuffer as an emulator-side output buffer only; hardware pixel production should conceptually flow through fetcher -> FIFO -> LCD output.
- The fetcher and pixel path should be able to grow future metadata such as bank source, palette selection, or priority-related information without redesigning the whole pipeline.
- Do not hard-code DMG palette mapping as the final renderer boundary; keep a stage where hardware pixel meaning can later expand for CGB palettes and tile attributes.
- Treat CGB palette and tile-attribute support as future extensions of the same pixel pipeline, not as a replacement renderer.
- SCX startup discard, window start behavior, and sprite fetch pauses must be able to delay pixel output and therefore stretch Mode 3 naturally.
- Window activation must be treated as a pipeline event that can change the tile source and extend Mode 3, not as a purely visual switch.
- Sprite handling during Mode 3 must be able to interrupt or stall the normal background fetch flow while object data is incorporated.
- Treat Mode 2 as a preparatory pipeline phase for Mode 3, not as an isolated bookkeeping pass.
- The list of visible sprites produced in Mode 2 should feed directly into Mode 3 object timing and mixing logic.
- STAT mode transitions should be modeled from the real dot schedule, not reconstructed after the scanline.
- Document and preserve the DMG-specific STAT write quirk when STAT behavior is implemented in detail; do not assume GBC-in-DMG-mode behaves identically.
- A `SkipBoot` path should synthesize internal LCD mode, dot position, and any relevant pipeline state coherently with the visible post-boot register snapshot instead of inventing a contradictory hidden phase.
- Do not present `OBP0` and `OBP1` as stable fixed post-boot values in DMG-family direct-boot presets; those registers should remain under an explicit uninitialized-state policy when firmware execution is skipped.
- Let the PPU define when VRAM/OAM are logically inaccessible, while the bus remains responsible for exposing the observable blocked-access result to other actors.
- Keep the PPU as the source of truth for whether VRAM or OAM are currently accessible, but let the bus enforce the resulting CPU-visible read/write behavior.
- Let the PPU raise LCD interrupt requests through the shared interrupt-controller path rather than owning `IF` state or dispatching CPU interrupt service directly.

## Known pitfalls

- STAT interrupt edge handling
- window trigger behavior
- sprite priority and timing shortcuts
- modeling DMA/PPU access conflicts too loosely
- baking DMG-only palette assumptions into the final pixel representation
- assuming Mode 3 starts with valid queued pixels and no pipeline startup cost
- forcing Mode 3 to a constant duration
- forcing HBlank to a constant duration independent of Mode 3 work
- pushing whole tiles or scanlines directly to a framebuffer without a FIFO model
- selecting sprites without respecting OAM order and the per-line limit of `10`
- modeling Mode 2 as an instant scan instead of a fixed `80`-dot phase
- treating STAT behavior as a generic interrupt source without hardware-specific LCD quirks
- synthesizing `SkipBoot` LCD registers without a matching hidden PPU phase

## Open questions

- the exact level of sprite-fetch and window-trigger detail required for the first DMG milestone
