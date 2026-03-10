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

## LCD MMIO contract baseline

- `LCDC` should remain owned by the PPU/LCD controller rather than by a generic MMIO byte bank.
- Writing `LCDC.7` should trigger immediate LCD/PPU side effects, including the LCD enable/disable transition and the corresponding VRAM/OAM accessibility rules.
- `STAT` should be modeled as a mixed register with writable interrupt-enable fields and dynamic read-only fields for coincidence and the current PPU mode.
- Preserve the documented DMG-specific spurious `STAT` interrupt quirk on `STAT` writes; do not assume the same write behavior on GBC running in DMG mode.
- `LY` should be read-only and reflect the current live scanline `0-153`; writes must not behave like storage updates.
- `LYC` is readable and writable storage, but its comparison effect belongs to the live PPU state and should be evaluated continuously against `LY`.
- `SCX`, `SCY`, `WX`, and `WY` should be modeled as MMIO-visible PPU registers whose mid-frame writes participate in the same temporal PPU model rather than a deferred renderer recomputation.
- `BGP`, `OBP0`, and `OBP1` should remain PPU-owned DMG palette registers.
- For `OBP0` and `OBP1`, the low two bits must not change the meaning of OBJ color index `0`, because that index remains transparent.

## Sprite pipeline baseline

- DMG sprites must be integrated into the real Mode 3 pixel pipeline rather than rendered by a scanline-level compositor layered over a finished background image.
- The PPU should keep separate BG FIFO and OBJ/OAM FIFO state and perform BG/OBJ mixing when pixels are popped for LCD output, not after the background has already been rendered.
- Sprite presence must be able to lengthen Mode 3 through real fetch pauses and object-fetch work rather than through a fixed per-scanline sprite surcharge.

## Sprite representation baseline

- OAM contains `40` sprite entries of `4` bytes each: `Y`, `X`, tile index, and attributes.
- Sprite coordinates should be interpreted using the hardware offsets `screen_y = oam_y - 16` and `screen_x = oam_x - 8`.
- A sprite with `Y = 0` or `Y >= 160` should be treated as vertically hidden.
- A sprite with `X = 0` or `X >= 168` should be treated as horizontally off-screen.
- In `8x16` mode, bit `0` of the tile index should always be ignored so the upper tile is even-aligned and the lower tile is the following odd tile.
- Per-sprite attributes should explicitly include at least BG-over-OBJ priority, X flip, Y flip, and DMG palette selection `OBP0` or `OBP1`.

## Mode 2 sprite-selection baseline

- Mode 2 should select candidate sprites for the current scanline using only the current `LY`, the sprite `Y` coordinate, and the live global size selected by `LCDC.2`.
- Sprite selection should ignore `X`; horizontally off-screen sprites still count toward the per-line selection limit if they match vertically.
- The selection order should be OAM order from `FE00` upward, stopping once `10` matching sprites have been collected.
- The current line's selected-sprite list should preserve OAM discovery order for later priority and timing work.

## DMG OBJ priority baseline

- Keep selection priority and drawing priority conceptually separate.
- On DMG, selection priority is OAM traversal order under the hard `10`-sprite-per-line limit.
- On DMG, drawing priority between overlapping non-transparent OBJ pixels should prefer the smaller `X` coordinate.
- If overlapping OBJ pixels have the same `X`, drawing priority should prefer the earlier OAM entry.
- Do not reuse CGB's OBJ-priority policy for DMG OBJ/OBJ conflicts.

## OBJ transparency and BG/OBJ mixing baseline

- OBJ color index `0` must always be treated as transparent.
- The object FIFO should support transparent filler pixels of minimal priority so partially available object runs do not accidentally block the background.
- Transparent OBJ pixels must not be treated as visible white output; transparency is a property of the object color index before DMG palette mapping.
- The PPU should resolve OBJ/OBJ priority first, producing one winning object pixel candidate, and only then apply the BG-versus-OBJ rule using that winning pixel's attributes.
- The OBJ BG-over-OBJ attribute must not influence which sprite wins an OBJ/OBJ overlap.
- A higher-priority OBJ may therefore hide a lower-priority OBJ even if the winning OBJ later stays behind a nonzero BG pixel.
- BG/OBJ mixing should be decided per popped pixel using the live BG pixel, the winning OBJ pixel, `LCDC.0`, `LCDC.1`, and the OBJ priority attribute.
- In DMG mode with `LCDC.0 = 0`, BG and window output should be forced to white while OBJ output can still remain visible when `LCDC.1 = 1`.
- A BG pixel of color `0` should not block an eligible OBJ pixel as if the BG were opaque.

## Object fetch and stall baseline

- Mode 3 should include an explicit object-fetch path instead of treating sprite timing as a scalar penalty.
- Object fetch should be able to wait for the BG fetcher to reach the relevant point before the sprite data is incorporated into the object FIFO.
- Sprite fetch work should be able to stall pixel output and lengthen Mode 3 on the shared dot timeline.
- The special DMG timing penalty involving `SCX & 7 > 0` together with a sprite at `X = 0` should have an explicit path in the design even if the exact timing remains documented as partially unsettled.
- Avoid reducing sprite timing to "add N dots per sprite" without internal fetcher state.

## Mid-frame toggle and size-change baseline

- `LCDC.1` should be observable by the sprite pipeline mid-frame, including during an in-flight object fetch.
- If `LCDC.1` is turned off during active object fetching, the design should support an explicit fetch-cancel path with real timing cost rather than a pure visibility flag change.
- `LCDC.2` sprite size should be treated as live state, not as a once-per-frame configuration snapshot.
- In `8x16` mode, line selection and tile-row calculation should treat the sprite as two stacked tiles with even/odd tile pairing derived from the masked tile index.
- Keep a dedicated task for the visible DMG artifacts and leaks caused by changing `LCDC.2` mid-frame, especially during the lower half of an `8x16` sprite.

## Sprite edge-case baseline

- Vertically clipped sprites near the top and bottom of the screen should be modeled explicitly rather than rejected as fully invisible.
- Cases such as `Y = 2` and `Y = 154` should remain part of the test plan because they expose partial-row visibility differences between `8x8` and `8x16`.
- A sprite hidden by `X = 0` or `X >= 168` should remain capable of consuming one of the `10` Mode 2 sprite-selection slots if its `Y` matches the current scanline.

## Sprite and window integration baseline

- Sprite mixing must remain compatible with a BG/window pipeline in which window start clears the BG FIFO and resets the fetcher mid-line.
- Do not design sprites as a layer applied over a fully resolved BG image, because window activation can restart fetch state in the middle of the scanline.
- Keep window-specific glitches and `WX`/`WY` behavior in their own explicit pipeline/state model so sprite logic stays decoupled while remaining compatible with detailed window behavior.

## Window pipeline baseline

- The window must be part of the same Mode 3 fetcher/FIFO pipeline as BG and sprites; it must not be modeled as a second background compositor applied after the scanline has already been built.
- Window start is a temporal pipeline event in the middle of the scanline rather than a frame-level or scanline-level mode switch.
- When the window starts rendering, the BG FIFO should be cleared and the fetcher should restart from its initial fetch step.
- Window visibility must depend on the combined state of `LCDC.5`, the WY latch, and the runtime WX trigger rather than on a single late visibility flag.
- Window Y addressing must come from a dedicated internal window line counter rather than from naïvely using `LY - WY` at all times.

## Window coordinate baseline

- `WX` should be interpreted using the hardware `X + 7` convention rather than as a direct screen X coordinate.
- `WY` should be treated as the visible starting scanline for window activation, not as a generic continuously applied Y offset.
- The window should only be considered potentially visible when `WX` is within `0..=166` and `WY` is within `0..=143`.
- `WX = 0` and `WX = 166` should remain explicit edge-case paths because they have distinct DMG-visible glitches.

## Window activation baseline

- The WY condition should be checked at the start of Mode 2 for each scanline and latched for later Mode 3 use.
- Once latched for a given scanline, the WY condition should not be recomputed continuously during the same line.
- The WX condition should be evaluated during pixel production using the current render position of the pipeline.
- The window should start on a scanline only when the WY latch is active, the WX trigger point is reached, and `LCDC.5 = 1`.
- In DMG mode, `LCDC.0 = 0` should suppress window rendering even if `LCDC.5 = 1`.

## Window tilemap and fetch baseline

- The window tilemap should be selected by `LCDC.6`, independent of the BG tilemap selection.
- Window tile data addressing should follow `LCDC.4`, matching BG tile addressing rules while remaining separate from OBJ tile handling.
- The fetcher should have explicit BG and window fetch modes rather than reusing BG fetch implicitly through altered coordinates.
- Window tile X should derive from a window-local X counter, not from `SCX`.
- Window tile Y should derive from the internal window line counter, not from `LY + SCY`.
- BG-side `SCX` and `SCY` rereads per tile fetch should remain confined to BG coordinate logic and must not leak into window fetch coordinates.

## Window start-event baseline

- Starting the window should clear the BG FIFO.
- Starting the window should reset the fetcher to its initial fetch step rather than continuing from the current BG fetch phase.
- The window-start event should alter the remaining pixel sequence of the current scanline without replaying or recomputing the whole line.
- The DMG special case `WX = 0 && (SCX & 7) > 0` should be modeled as an explicit path that shortens Mode 3 by `1` dot.

## Window line-counter baseline

- The PPU should keep an explicit internal window line counter.
- That counter should reset during VBlank.
- The counter should increment only on scanlines where the window actually begins rendering.
- Hiding the window mid-frame via `WX` manipulation or `LCDC.5` should be able to prevent the increment for affected lines.
- Do not define the window row globally as `LY - WY`; that shortcut is not valid for status bars and mid-frame show/hide behavior.

## Window mid-frame write and glitch baseline

- Writes to `WX`, `WY`, and `LCDC.5` during the frame must be visible to the live pipeline rather than deferred until the next frame.
- If the WY latch is already active for the current line and `LCDC.5` was active at line start but is cleared before the WX trigger point, the design should support the documented window-glitch pixel at the would-be window start.
- If `WX` changes after the window has already started on the line and the new trigger position is reached again, the documented bug should be representable as a low-priority color-`0` pixel pushed into the BG FIFO path.
- These glitches should live in fetcher/FIFO/pipeline logic rather than as framebuffer post-processing rules.

## Window edge-case baseline

- `WX = 0` should be treated as a DMG-specific special case whose visible stutter depends on `SCX & 7`, not as a normal "starts at X = -7" case.
- `WX = 166` should retain its special behavior of extending across the following scanline rather than being clipped away as an ordinary out-of-range value.
- `WX = 0` and `WX = 166` should each have their own explicit tests and implementation paths.

## Window and sprite interaction baseline

- Window start should change the BG/window pixel stream before OBJ-versus-BG mixing for the final LCD pixel.
- Starting the window should not automatically clear the OBJ/OAM FIFO; the documented reset is on the BG-side FIFO path.
- Window glitches must be able to affect final sprite mixing because they alter the actual BG/window pixels consumed by the mixer.

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
- tests for Mode 2 sprite selection using Y only, including horizontally off-screen sprites still consuming one of the `10` slots
- tests for DMG OBJ/OBJ priority: lower `X` wins, then OAM order on equal `X`
- tests for OBJ color `0` transparency and transparent object FIFO filler behavior
- tests for BG/OBJ mixing using the winning OBJ pixel before applying the BG-over-OBJ rule
- tests for `8x8` versus `8x16` selection and row mapping, including bit `0` ignored on `8x16` tile indices
- tests for top-edge and bottom-edge partial sprite visibility such as `Y = 2` and `Y = 154`
- tests for mid-frame `LCDC.1` and `LCDC.2` changes when relevant behavior is implemented or intentionally isolated
- tests for WY latch timing at Mode 2 start and WX-trigger timing during Mode 3
- tests for window fetcher reset and BG FIFO clear when the window starts mid-scanline
- tests for the internal window line counter, including increment-only-when-started and reset during VBlank
- tests for `WX = 0` and `WX = 166` special behavior
- tests for DMG `LCDC.0` suppressing window rendering even when `LCDC.5 = 1`
- tests for mid-frame `WX`, `WY`, and `LCDC.5` writes when relevant glitches are implemented or intentionally isolated
- tests for window-start and window-glitch cases that continue into later BG/OBJ mixing without resetting the OBJ/OAM FIFO incorrectly
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
- A shape such as `SelectedSpritesForLine`, `ObjectFetcherState`, `OamFifo`, `ObjectPixel`, `SpritePriorityResolver`, and `BgObjMixer` is a good fit for keeping sprite work explicit and testable.
- A shape such as `wy_triggered`, `window_active_this_line`, `window_line_counter`, `window_x_counter`, explicit window-start events, and pending WX/LCDC-related glitch state is a good fit for keeping window behavior explicit and testable.
- A fetcher-source distinction such as `BackgroundFetch` versus `WindowFetch` is preferred over late coordinate branching at mix time.
- Use one consistent local term for the sprite-pixel queue; `OBJ FIFO`, `OAM FIFO`, and `ObjectFifo` in this documentation refer to the same hardware-facing queue.
- The object FIFO should carry per-pixel metadata such as color index, palette selection, OBJ priority attribute, X-priority information, OAM-order tie-break information, and transparency.
- Keep OBJ/OBJ priority resolution separate from BG/OBJ mixing so the BG-over-OBJ attribute is applied only after the winning sprite pixel has been chosen.
- Apply X flip, Y flip, palette selection, and `8x16` tile-row mapping during object fetch and FIFO population rather than as a framebuffer post-process.
- The BG-to-window fetch transition should be represented as an explicit pipeline event rather than as a late conditional in the pixel mixer.
- STAT mode transitions should be modeled from the real dot schedule, not reconstructed after the scanline.
- Document and preserve the DMG-specific STAT write quirk when STAT behavior is implemented in detail; do not assume GBC-in-DMG-mode behaves identically.
- Mid-frame writes to LCD-visible registers should be interpreted on the same dot timeline that drives mode, fetcher, FIFO, and interrupt behavior.
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
- modeling the window as a second background layer composited after the scanline instead of as a fetcher/FIFO transition
- deriving the window row solely from `LY - WY` and thereby breaking mid-frame hide/show behavior
- resetting the whole scanline instead of only the relevant fetcher/FIFO state when the window starts
- ignoring `WX = 0`, `WX = 166`, or mid-frame `WX`/`WY`/`LCDC.5` writes because they are rare edge cases
- resolving BG/OBJ mixing before resolving which OBJ pixel actually wins an overlap
- using X visibility as part of Mode 2 sprite selection and thereby hiding real `10`-sprite-per-line exhaustion
- treating OBJ color `0` as white output instead of transparency
- collapsing sprite timing into a constant per-sprite penalty with no explicit object-fetch state
- treating STAT behavior as a generic interrupt source without hardware-specific LCD quirks
- storing `LY` as a writable register instead of exposing the live scanline
- letting LCD-visible MMIO writes bypass the temporal PPU model and only affect a later renderer pass
- synthesizing `SkipBoot` LCD registers without a matching hidden PPU phase

## Open questions

- the exact level of sprite-fetch and window-trigger detail required for the first DMG milestone
