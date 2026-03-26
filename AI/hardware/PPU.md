# PPU

## Scope

Own LCD/PPU mode progression, rendering state, VRAM/OAM access rules, STAT behavior, fetcher/FIFO behavior, and frame output generation.

## Hardware model

Model PPU modes explicitly. Separate fetcher/FIFO logic when it improves clarity and timing fidelity.

Even in DMG-only work, avoid hard-wiring the design to a single permanent VRAM interpretation or a renderer that only understands four grayscale outputs.
For this project, the PPU should be modeled dot-by-dot, where `1 dot = 1 T-cycle`.

## Responsibilities

- mode transitions and scanline progression
- current Mode `2` OAM-row progression
- background, window, and sprite fetch behavior
- pixel priority rules
- STAT/LY/LYC and LCD-visible interrupts
- DMG-family OAM corruption behavior
- reaction to DMA-declared OAM/VRAM contention without owning DMA scheduling
- consumption of bus-originated OAM/VRAM domain views rather than unrelated raw backing arrays
- consumption of bus-synchronized OAM/VRAM ownership state; the PPU must not invent video-bus acquisition or release transitions locally
- explicit separation between MMIO-owned register storage and the register values currently visible to the active pixel pipeline
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

## DMA interaction baseline

- The PPU should not infer live DMA behavior from `FF46` or future `HDMA1-5` register contents.
- The DMA subsystem should publish a common current-cycle memory-region impact such as `Oam`, `Vram`, or no special region, and the PPU should consume that signal when concurrent transfer activity affects PPU-visible behavior.
- OAM DMA active state and duration belong to DMA; the PPU keeps ownership of the visible consequences such as OAM read failure, Mode `2/3` interaction, and DMG-family OAM corruption behavior.
- For DMG OAM DMA, keep the coarse "DMA is currently blocking OAM" signal separate from the finer same-cycle destination-word hint used by late Mode `3` sprite-metadata conflicts; the PPU needs both views.
- Future HBlank-conditioned transfers should use the PPU's live mode or HBlank-visible state as an input to DMA advance conditions without moving HDMA scheduling logic into the PPU or the bus.

## LCD MMIO contract baseline

- `LCDC` should remain owned by the PPU/LCD controller rather than by a generic MMIO byte bank.
- Writing `LCDC.7` should trigger immediate LCD/PPU side effects, including the LCD enable/disable transition and the corresponding VRAM/OAM accessibility rules.
- `STAT` should be modeled as a mixed register with writable interrupt-enable fields and dynamic read-only fields for coincidence and the current PPU mode.
- Preserve the documented DMG-specific spurious `STAT` interrupt quirk on `STAT` writes; do not assume the same write behavior on GBC running in DMG mode.
- `LY` should be read-only and reflect the current live scanline `0-153`; writes must not behave like storage updates.
- `LYC` is readable and writable storage, but its comparison effect belongs to the live PPU state and should be evaluated continuously against `LY`.
- `SCX`, `SCY`, `WX`, and `WY` should be modeled as MMIO-visible PPU registers whose mid-frame writes participate in the same temporal PPU model rather than a deferred renderer recomputation.
- `BGP`, `OBP0`, and `OBP1` should remain PPU-owned DMG palette registers.
- The implementation should keep one explicit current-dot-visible register block for active-LCD fetch and pixel mixing. In the current DMG baseline, that visible block may lag the MMIO-owned storage by one shared T-cycle so writes committed after the PPU tick become visible on the next PPU dot instead of retroactively changing the fetch already in progress.
- That visible-register block should be the source of truth for Mode `3` BG/window/object fetch decisions, BG/OBJ palette lookup, and other active-pipeline reads of `LCDC`, `SCX`, `SCY`, `WX`, `WY`, `BGP`, and `OBP*`.
- For `OBP0` and `OBP1`, the low two bits must not change the meaning of OBJ color index `0`, because that index remains transparent.
- On DMG-family hardware, writes to `BGP`, `OBP0`, and `OBP1` during Mode `3` should not be treated as ordinary "new value is visible only from the next pixel onward" MMIO updates. The PPU design should leave room for the documented/raster-oracle-visible palette-conflict artifacts, including transient write values and limited retroactive recoloring of the most recent visible pixels when those writes race the LCD pipeline.
- That DMG palette-conflict window should also remain compatible with the observed early-HBlank tail used by raster tests such as `mealybug m3_bgp_change`; do not hard-cut those writes to "no effect" merely because the mode bits already advanced to HBlank in the coarse scheduler model.
- The current DMG baseline should not assume `BGP` and `OBP*` share exactly the same retroactive span. Current mealybug evidence in this repo supports a slightly wider retroactive OBJ-palettes window than the BG-palettes window, so keep the two cases separable in the design instead of baking one universal pixel count into every palette-write path.

## LCD master-control baseline

- `LCDC.7` should be treated as the master enable for the LCD/PPU subsystem, not as a late visibility-only flag on an otherwise running raster.
- A transition `LCDC.7: 1 -> 0` should disable the active LCD/PPU pipeline itself rather than merely hiding already-generated pixels.
- A transition `LCDC.7: 0 -> 1` should re-enable the PPU immediately on the shared T-cycle timeline, without an invented delay before internal drawing resumes.
- CPU execution, timer progress, DMA, and interrupt logic should continue normally while the LCD/PPU is disabled; LCD off is not a whole-machine pause.
- Mid-scanline writes to `LCDC.7` should take effect on the access itself unless later hardware evidence proves a narrower timing rule.

## LCD-off state baseline

- LCD-disabled state should be represented as a dedicated PPU/LCD-disabled condition, not as the ordinary HBlank path with rendering silently hidden.
- When `LCDC.7 = 0`, the PPU should stop progressing through the ordinary active-LCD mode schedule.
- The same disabled-state decision should drive both `STAT.mode = 0` readback and the bus-side release of normal VRAM/OAM mode restrictions.
- Releasing LCD-mode access restrictions must not erase independent bus-side restrictions such as active OAM DMA; LCD-off policy and DMA policy must still compose cleanly.
- LCD-disabled state should therefore be one explicit source of truth consumed by both the PPU-visible register contract and the bus access-policy contract.

## LCD-visible output baseline

- The project should distinguish between the internal PPU pixel pipeline and the visible LCD-panel output state.
- When the LCD is disabled, the visible output should be forced to the LCD-off DMG white state rather than to palette color `0` as if the PPU were still presenting ordinary pixels.
- After re-enabling the LCD, the PPU may resume internal drawing immediately while the panel-visible output remains forced blank for the first full frame.
- The "first full frame stays blank" rule should be modeled as visible-output behavior, not as a delayed start of the internal PPU scheduler.

## STAT register baseline

- `STAT` should remain a mixed register whose writable portion is the software-configured interrupt-enable mask and whose read-only portion is derived from live PPU state.
- In the current DMG-family baseline, `STAT` bit `7` should read back as `1`; it is not a writable software-owned bit.
- Bits `6-3` should be treated as writable enables for the `LYC==LY`, Mode `2`, Mode `1`, and Mode `0` STAT sources.
- Bit `2` should expose the live `LYC==LY` coincidence state as a read-only flag.
- Bits `1-0` should expose the live current PPU mode as a read-only value: `0` HBlank, `1` VBlank, `2` OAM scan, `3` drawing.
- When the LCD/PPU is disabled through `LCDC.7 = 0`, `STAT` mode bits should read back as `0`.
- Writes to `STAT` must not overwrite the live mode bits or the coincidence flag.

## LY / LYC coincidence baseline

- `LY` should advance through the live scanline range `0..=153`, including `144..=153` during VBlank.
- On DMG-family timing, the bus-facing `FF44` readback should advance to the next scanline during the last machine cycle of HBlank before the full raster wrap completes; do not force bus-visible `LY` reads to be identical to the internal raster/comparison line at every dot.
- The `LYC==LY` flag should come from a continuous comparison between the live `LY` and `LYC` values, not from a once-per-line event cache.
- Writing `LYC` should immediately reevaluate the live coincidence state rather than waiting for the next scanline boundary.
- While the LCD is disabled, the `STAT` coincidence bit should retain the last active-LCD comparison result instead of being silently recomputed from the reset LCD-off `LY = 0` state.
- Writing `LYC` while the LCD is disabled should update the stored compare target only; it must not recompute that retained LCD-off coincidence result or request LCD STAT by itself.
- Coincidence should remain possible during VBlank as well as during visible scanlines.
- The PPU should not model `LYC` as "schedule a future interrupt when LY reaches this line"; it is a live comparison input to `STAT`.

## STAT interrupt-line baseline

- The PPU should keep an explicit internal `stat_irq_line` or equivalent signal distinct from the visible `STAT` byte readback.
- That internal line should be computed as the OR of the enabled live sources:
  - `stat_mode0_enable && mode == 0`
  - `stat_mode1_enable && mode == 1`
  - `stat_mode2_enable && mode == 2`
  - `stat_lyc_enable && ly == lyc`
- In the current DMG-family baseline, the Mode `2` STAT enable should also request LCD STAT at the exact VBlank-entry transition on line `144`, but it should not be treated as a continuously active source for the rest of VBlank.
- LCD STAT interrupt requests should be emitted only on a rising edge of that internal line, not merely because one contributing condition is true.
- STAT blocking should be preserved: if one enabled source keeps the internal line high while another source becomes true, no new LCD STAT interrupt should be requested until the line first drops low and rises again.
- Mode `3` must not be treated as a direct STAT interrupt source.

## STAT scheduling and interrupt baseline

- `STAT` mode changes should come directly from the real PPU dot scheduler rather than from a post-hoc per-scanline summary.
- Entry into Mode `2`, Mode `3`, Mode `0`, and Mode `1` should become visible to `STAT` at the real dot where the PPU scheduler changes mode.
- Because Mode `3` is variable-length, the exact Mode `3 -> 0` transition point must propagate to the `STAT` line and LCD interrupt timing without being quantized to a fixed scanline template.
- Keep room for the internal LCD STAT interrupt line to lead the readable `STAT.mode` bits by a few T-cycles on DMG-family hardware. Current oracle-backed closure in this repo requires the Mode `0` STAT source to be able to rise up to `4` dots before HBlank becomes visible through `STAT` readback and before VRAM bus release.
- Entering VBlank at `LY = 144` should be able to request both the dedicated VBlank interrupt and the LCD STAT interrupt for Mode `1` independently when the corresponding `STAT` enable is set.
- The same live mode state that feeds `STAT` must also feed VRAM/OAM accessibility decisions so software polling `STAT` sees the same timing the bus uses for blocking.
- On the shared scheduler, the PPU dot tick should happen before current-cycle bus arbitration and interrupt aggregation so `STAT`, LCD IRQ requests, `LY`, and VRAM/OAM restrictions remain coherent for that T-cycle.

## STAT write quirk baseline

- On DMG-family hardware, writing `STAT` during Mode `0`, Mode `1`, Mode `2`, or while `LYC==LY` is true should support the documented spurious LCD STAT interrupt behavior.
- That quirk should be modeled as a temporary elevation-equivalent effect on the internal STAT interrupt line rather than as "every write to `STAT` requests an interrupt."
- The quirk must not trigger from a Mode `3` write path merely because `STAT` was written.
- Future GBC-in-DMG-mode support must keep this quirk model-gated rather than inheriting the DMG behavior accidentally.

## LCD disable / re-enable baseline

- When `LCDC.7 = 0`, the PPU should stop behaving as if it were traversing ordinary Modes `2`, `3`, `0`, and `1` in the background.
- With LCD disabled, `STAT` mode should report `0`, VRAM should become ordinarily accessible again, and OAM should follow the same LCD-off policy already used by the bus while still remaining compatible with separate DMA-side blocking rules.
- The internal STAT interrupt line should stop following the ordinary active-LCD mode/coincidence-source schedule while LCD is disabled.
- The disabled-state transition should also clear or recompute any previous `stat_irq_line` edge-detection state so LCD re-enable does not inherit a stale-high STAT source.
- Re-enabling LCD should restart the PPU timing state through the real scheduler path and remain compatible with the separate rule that the first full frame after re-enable stays blank.

## LCD re-enable and raster restart baseline

- Re-enabling the LCD should enter one explicit, reproducible raster-start state rather than resuming from an ambiguous saved dot or half-finished scanline.
- The implementation should keep one source of truth for the initial scanline, dot, mode, and related scheduler state used after `LCDC.7: 0 -> 1`.
- In the current DMG-family baseline, `STAT` should expose one short initial Mode `0` readback window immediately after LCD re-enable before ordinary raster mode reporting resumes; full first-line restart timing still needs its own oracle closure.
- The first-full-frame blank period should be counted from that re-enabled raster start, not from the earlier disable event.
- The implementation should also keep one explicit, tested policy for how `LY` behaves while the LCD is disabled and how it re-enters the active raster model after re-enable.

## LCD pipeline reset baseline

- Disabling the LCD should explicitly invalidate or reset in-flight pixel-pipeline state rather than freezing and later resuming a half-consumed scanline.
- That reset should cover at least BG FIFO state, OBJ/OAM FIFO state, background/window fetcher state, object-fetch state, window latch/counter state, and any in-progress pixel-mixing state.
- Re-enabling the LCD should start pixel production from a clean pipeline state compatible with the chosen raster-start state.
- Do not resume fetchers or FIFOs from the last active-LCD dot before disable; that would contradict the hardware-facing model of the PPU being off and then starting a new draw again.

## LCD-disabled scheduler / IRQ baseline

- While the LCD is disabled, mode-dependent LCD STAT sources should not continue to fire as if the raster were still advancing invisibly.
- The `STAT` controller, `LY` handling, and any mode-driven PPU scheduler state should move coherently into the same explicit LCD-disabled state.
- The implementation should not let `LY` keep advancing accidentally just because a generic line counter happened to keep ticking after the LCD was turned off.
- Re-enabling the LCD should rebuild live coincidence and STAT-line state from the chosen raster restart state rather than reusing stale coincidence or edge-detection state from before disable.
- If the project later offers a debug warning for disabling the LCD outside VBlank, that warning must remain observational only and must not change the emulated hardware result.

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
- On DMG-family hardware during active OAM DMA, blocked Mode `2` OAM reads should reuse the last latched OAM word instead of inventing fresh `Y/X` values or force-clearing selection.
- That stale Mode `2` word should remain shared with later OAM reads such as Mode `3` sprite metadata fetches, so the next scanline's DMA-blocked Mode `2` path can inherit the most recent latched word even when it came from tile/attribute reads rather than from an earlier `Y/X` scan.

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
- BG/window/object fetch helpers should consume `VramBusView` / `OamBusView`-style domain clients rather than unrelated `&[u8]` slices so future bus-ownership, storage, and CGB-bank changes do not force another PPU boundary rewrite.
- Late Mode `3` sprite metadata reads should come from live OAM rather than from a frozen Mode `2` metadata snapshot.
- During DMG OAM DMA, that late metadata path should be able to read the DMA destination word being written on the current cycle instead of the nominal sprite metadata address, because tests such as `hacktix/strikethrough` depend on that conflict window.

## Mid-frame toggle and size-change baseline

- `LCDC.1` should be observable by the sprite pipeline mid-frame, including during an in-flight object fetch.
- If `LCDC.1` is turned off during active object fetching, the design should support an explicit fetch-cancel path with real timing cost rather than a pure visibility flag change.
- `LCDC.2` sprite size should be treated as live state, not as a once-per-frame configuration snapshot.
- In `8x16` mode, line selection and tile-row calculation should treat the sprite as two stacked tiles with even/odd tile pairing derived from the masked tile index.
- If a live `LCDC.2` size change shrinks a previously selected sprite so that the current scanline row falls outside the new height, keep that out-of-range case explicit instead of letting row arithmetic underflow; the baseline may currently resolve that fetch as no OBJ data while preserving the rest of the timing path until oracle-backed artifact closure lands.
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
- In the current DMG baseline, treat the window trigger itself as one explicit activation dot before the restarted window fetcher begins advancing again; do not let the restart behave as if the first window fetch step had already consumed time on the same activation dot.
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

## OAM corruption bug baseline

- The DMG-family OAM corruption bug should be modeled as hardware behavior tied to the live PPU/CPU/bus interaction, not as a short opcode blacklist or a generic "weird OAM write" exception.
- The affected hardware family should include at least `DMG0`, `DMG`, and `MGB`, while the architecture should stay ready to extend the same model to `SGB` and `SGB2`.
- Future `CGB`, `AGB`, `AGS`, and `GBP` support must not inherit this DMG-family bug automatically, even when running monochrome software.
- During Mode `2`, the PPU should expose the currently scanned OAM row as explicit state. That row is an `8`-byte slice, and Mode `2` should advance through the `20` rows at one row per `4` dots (`1` M-cycle as a descriptive grouping only).
- OAM corruption logic must therefore consume both the current live PPU mode and the current Mode `2` row instead of relying only on a coarse "OAM blocked" flag.
- The first OAM row, `FE00-FE07` (objects `0` and `1`), should remain immune to the basic corruption patterns documented for the bug.

## OAM corruption trigger baseline

- Two trigger families should remain distinct:
  - CPU-visible read or write attempts to OAM during Mode `2`, including reads from `FEA0-FEFF` on affected DMG-family hardware
  - `16`-bit increment/decrement activity whose address output lies in `FE00-FEFF`, because the IDU drives that value onto the address bus even without an ordinary memory read or write
- Do not model the bug as a fixed list of affected opcodes. The trigger source is the microarchitectural event: read, write, read plus `inc/dec`, or write plus `inc/dec`.
- The event model must be rich enough to cover ordinary OAM accesses, `inc rr` / `dec rr`, `[hli]` / `[hld]`, `push` / `pop`, `call` / `ret` / `rst`, interrupt service, and `PC` increments while executing from OAM.
- The pattern must not depend on the exact byte address touched inside OAM, nor on the data value written by the CPU; it depends on the current Mode `2` row and the trigger class.

## OAM corruption pattern baseline

- Corruption should be reasoned about in `16`-bit OAM words, not as independent byte noise.
- A write-triggered corruption on the current row, except for row `0`, should:
  - replace the first word of the current row with `((a ^ c) & (b ^ c)) ^ c`
  - copy the last three words of the previous row into the last three words of the current row
- In that write pattern, `a` is the current row's first word before corruption, `b` is the previous row's first word, and `c` is the previous row's third word.
- A read-triggered corruption should keep its own first-word formula, `b | (a & c)`, while preserving the same "copy the previous row's last three words" structure.
- `write + inc/dec` in the same effective CPU step should behave as one effective write corruption, not as two stacked write corruptions.
- `read + inc/dec` should have its own dedicated path rather than being synthesized from "read corruption plus write corruption". That complex path should remain gated off for the first four rows and for the last row, matching the documented row exceptions.
- In the complex `read + inc/dec` path, let `a` be the first word two rows before the current row, `b` the first word of the previous row, `c` the first word of the current row, and `d` the third word of the previous row. The first word of the previous row should first become `(b & (a | c | d)) | (a & c & d)`.
- After that first-word mutation, the whole previous row should be copied both into the current row and into the row two rows before the current row.
- After the row-restricted complex path resolves, the controller should still apply the ordinary read-corruption step for the current row in the same event sequence.

## OAM corruption controller baseline

- The project should keep an explicit `OamCorruptionController` or equivalent owner for the deterministic corruption formulas.
- That controller should consume at least:
  - active console model
  - whether the PPU is in Mode `2`
  - the current Mode `2` OAM row
  - an event kind such as `read`, `write`, `read_plus_incdec`, or `write_plus_incdec`
  - access to the underlying OAM storage in a word-oriented view
- The PPU should own the current-row source of truth, the bus should detect address-based triggers, and the CPU should expose the micro-operation events needed to classify IDU-driven cases.
- Do not duplicate the corruption formulas across CPU, bus, and PPU helpers.

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
- DMG-family OAM corruption should be tied to the exact T-cycle where the triggering access or IDU event occurs, using the Mode `2` row active in that `4`-dot slice.
- Do not generalize the OAM corruption bug to generic OAM blocking in Mode `3`; the documented bug is a Mode `2` plus IDU phenomenon.

## Dependencies

- bus and memory
- CPU
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
- tests for live `STAT` readback composition: forced-high bit `7`, writable enable bits, plus live mode and coincidence bits
- tests for `LY` covering `0..=153`, including `LYC` matches at `144`, `153`, and the `153 -> 0` wrap
- tests for immediate `LYC` write reevaluation of `STAT.2` and the internal STAT interrupt line
- tests for each enabled LCD STAT mode source path for Mode `0`, Mode `1`, and Mode `2`
- tests for the line-start Mode `2` / LCD STAT chronology used by raster-effect ROMs, including the non-line-`0` pretrigger path and first-line timing differences when a handler writes back into PPU MMIO during the same scanline
- tests for LCD STAT rising-edge behavior and STAT blocking across consecutive enabled sources such as Mode `0` followed by Mode `1`
- tests that Mode `3` never acts as a direct LCD STAT interrupt source
- tests that entering Mode `1` can request both VBlank interrupt and LCD STAT interrupt independently
- tests for DMG-family `STAT` write quirk in Mode `2`, Mode `0`, Mode `1`, and coincidence-active cases, plus a negative test for Mode `3`
- tests that the mode reported through `STAT` matches the same live state used by the bus to block or allow VRAM/OAM access
- tests for LCD off/on behavior around `STAT`, including LCD-off `STAT` mode readback, release of ordinary LCD-mode VRAM/OAM restrictions, and re-enable without stale STAT-line or coincidence carry-over
- tests for `LCDC.7: 1 -> 0` causing immediate LCD/PPU disable, visible white output, and release of ordinary VRAM/OAM mode restrictions
- tests for `LCDC.7: 0 -> 1` causing immediate internal PPU restart while keeping the visible output blank for the first full frame
- tests that LCD disable resets pipeline state so re-enable does not resume a corrupted partial scanline
- tests for one explicit LY/off/re-enable policy covering the disable point, steady LCD-off state, and re-enable boundary rather than accidental continued line counting during LCD-disabled state
- tests that mid-scanline `LCDC.7` writes take effect immediately rather than waiting for scanline or frame end
- tests that the Mode `2` OAM row exposed by the PPU is deterministic and advances one row per `4` dots
- tests for OAM corruption trigger families: ordinary OAM access, `FEA0-FEFF` read during Mode `2`, and IDU-driven `inc/dec` events in `FE00-FEFF`
- tests for first-row immunity of the basic OAM corruption patterns
- tests for write corruption and read corruption using the documented deterministic word formulas rather than random damage
- tests for `write + inc/dec` collapsing to one effective write-corruption path
- tests for the dedicated `read + inc/dec` pattern, including its row exclusions for the first four rows and the last row
- tests that `[hli]` / `[hld]`, `push` / `pop`, `call` / `ret` / `rst`, interrupt service, and executing code from OAM can all trigger the bug through the same event model
- tests that DMG-family models are affected while future CGB-family models are not
- direct-boot continuity tests that verify the first LCD-visible dots after `SkipBoot` are coherent with the published post-boot `LCDC`, `STAT`, and `LY` snapshot

## Implementation notes for this repo

- Keep mode state explicit.
- Separate rendering backend concerns from internal PPU state.
- Do not implement the PPU as a scanline renderer or a mode-only renderer if the goal is dot-by-dot / T-cycle-level accuracy.
- Mode 2 should be an explicit PPU state with its own dot counter, OAM traversal progress, and temporary visible-sprite list.
- In the current Phase `4.2` baseline, advance Mode `2` OAM traversal one sprite entry every `2` dots while preserving the fixed `80`-dot mode duration and carrying the resulting selected-sprite list forward for the rest of the line.
- In the current Phase `4.3` baseline, keep BG-only Mode `3` as one explicit fetcher-plus-FIFO pipeline with a `12`-dot startup cost, one BG pixel popped per dot after startup, and HBlank entry delayed by the latched `SCX & 7` startup discard for that line.
- In the current Phase `4.4` baseline, latch the WY condition at Mode `2` start, restart the shared BG/window fetcher plus clear the BG FIFO when the live WX trigger fires, increment the internal window line counter only on lines where window rendering actually began, and keep explicit provisional edge paths for `WX = 0` and `WX = 166`.
- In the current Phase `4.5` baseline, model OBJ work as an explicit `Startup -> TileDataLow -> TileDataHigh -> Push` fetch path with its own FIFO, resolve DMG OBJ/OBJ priority while populating that FIFO using lower `X` then OAM order, and perform BG/OBJ mixing only when the winning OBJ pixel and the current BG/window pixel are popped for LCD output.
- Keep the earliest-visible-sprite timing path explicit. The current baseline no longer clamps the left edge to a single "visible X = 0" trigger; it keeps a distinct pre-visible raw-`X` match path for early OBJ fetches so partially off-screen sprites can begin before the first visible BG pixel without collapsing every `X = 1..7` case into the same trigger point. For those low-`X` startup cases specifically, the OBJ fetch request now waits until the BG FIFO has been primed and the BG fetcher has moved past its initial `TileIndex` phase before the sprite path starts stealing dots, instead of freezing the whole line at the first match edge.
- In the current mealybug-driven DMG baseline, the palette-conflict approximation is also slightly asymmetric: `OBP0/OBP1` writes currently recolor one more recent visible OBJ pixel than `BGP` writes under the same coarse Mode `3` / early-HBlank conditions. Keep that asymmetry explicit rather than folding it back into the BG path.
- In the current Phase `4.6` baseline, keep one explicit internal LCD STAT line inside the PPU, recompute it from the live mode/coincidence state on every timing transition and relevant MMIO write, and request the LCD STAT interrupt only on `0 -> 1` edges of that line.
- Route both VBlank and LCD STAT requests out of the PPU through the shared interrupt-controller path on the scheduler timeline, and clear/reseed the internal STAT-line baseline across LCD off/on transitions so re-enable does not inherit stale-high edge state.
- In the current Phase `4.7` baseline, treat `LCDC.7` as a real LCD power transition: `1 -> 0` moves the PPU immediately into one explicit LCD-disabled state with `LY = 0`, `line_dot = 0`, cleared in-flight pipeline state, released ordinary LCD-mode bus restrictions, and panel-visible forced blank output.
- In the current Phase `4.7` baseline, `0 -> 1` restarts the internal raster immediately from one explicit DMG-family provisional entry state at line `0`, dot `4`, while the panel-visible output stays forced blank for the first full frame.
- The same restart path currently publishes one short early-dot `STAT.mode = 0` startup window after LCD re-enable before the ordinary raster-derived mode schedule resumes; model that window as one explicit raster state consumed consistently by `STAT`, mode-dot reporting, and Mode `2` gating rather than as scattered special-case guards. The finer restarted-line timing remains open.
- That restart state is still only partially closed: the retained LCD-off coincidence behavior and immediate re-enable `STAT` readback now satisfy `mooneye ppu/stat_lyc_onoff`, but the finer `LY/STAT` and OAM/VRAM boundary timing around `mooneye ppu/lcdon_timing-GS` and `ppu/lcdon_write_timing-GS` remains open and should not be treated as oracle-finished.
- In the current Phase `4.8` baseline, expose the live Mode `2` OAM row directly from the PPU raster as `line_dot / 4` over the fixed `80`-dot OAM-scan window, and keep that row unavailable outside visible-line Mode `2`.
- In the current Phase `4.8` baseline, keep one explicit `OamCorruptionController` inside the PPU, gate it to DMG-family models only, map pure IDU `inc/dec` events to the write-corruption path, and keep the dedicated `read + inc/dec` path restricted to rows `4..=18` before the ordinary read-corruption step is applied.
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
- In the current DMG baseline, the visible `Mode 3 -> 0` boundary should come from one line-local dot target owned by the live pixel-transfer state, updated on the exact dots where window restart or active OBJ fetch actually steal pixel-transfer time, instead of being reconstructed later from scanline-wide penalty counters.
- Window activation must be treated as a pipeline event that can change the tile source and extend Mode 3, not as a purely visual switch.
- Sprite handling during Mode 3 must be able to interrupt or stall the normal background fetch flow while object data is incorporated.
- In the current March 26, 2026 repo baseline, matching OBJ hits are now first latched into explicit pending per-line state instead of being rediscovered from one transient `current X` comparison. That pending-hit state is allowed to survive until the shared fetcher can actually begin the OBJ fetch.
- The same current baseline also requires the BG/window push-side state to carry an explicit "interrupted by object fetch" condition so an already fetched BG/window tile can remain cached across an intervening OBJ fetch rather than forcing a new tile read or silently losing the fetched slice.
- In the same current baseline, the BG/window `Push` state is now polled once per dot after an explicit one-dot entry delay instead of remaining tied to the generic 2-dot fetch-stage cadence forever. That keeps the repo's current first-push timing stable while allowing later push-side retries and OBJ arbitration to live on a per-dot state machine.
- In the same current baseline, push-side arbitration and BG FIFO refill are no longer the same internal event. The push phase first decides whether the fetched slice should wait, hand off to OBJ timing, or queue one explicit pending BG FIFO fill, and that queued fill is then materialized by a separate fill phase at the start of the following Mode `3` dot before OBJ fetch or pixel pop for that dot. Keep that separation explicit so future tightening toward stricter FIFO-empty push behavior does not require another state-boundary rewrite.
- The current March 26, 2026 baseline also keeps one explicit dot-local `Mode 3` phase order inside the PPU core: window-trigger handling for the current dot, then pending BG FIFO fill materialization, then active OBJ-fetch work if any, then BG pixel pop/discard/output work, and only after that the shared BG/window fetcher tick for the dot. Keep that ordering explicit so future docboy-style tightening can move one phase at a time instead of rewiring the whole line model again.
- In the same current baseline, the BG/window fetcher and OBJ fetcher should no longer advance through a generic "increment `stage_dot`, then maybe do work" helper. Treat the pair `stage + stage_dot` itself as the explicit one-dot automaton state for each fetcher, with each non-push fetch stage owning two concrete dot positions (`0` then `1`) and transitions expressed directly in the code. That keeps the live snapshot contract stable while making later dot-accurate timing changes local to one state edge at a time.
- In the same current baseline, once a sprite hit has already been latched for the current dot but the OBJ fetch cannot yet begin, the output side should not keep advancing `current X` as if nothing were pending. Keep one explicit output-side gate for that state: BG pop/pre-visible X advance must stall for the dot, and the line-local `Mode 3 -> 0` target must extend by one dot, until the pending OBJ hit is actually consumed or OBJ rendering becomes disabled.
- Keep the pending-hit queue tied to the `current X` that produced it instead of treating it as an unscoped bag of future OBJ work. If OBJ rendering is disabled or the pixel-transfer side advances to a different `current X`, any still-pending hits owned by the old `current X` must be discarded as stale rather than being allowed to start a late OBJ fetch on a later dot.
- In the same current baseline, the output side should distinguish "this dot was really served by the transfer side" from "the output phase merely ran". The eight hidden pre-visible startup dots before the first BG pop and later BG-pop / `SCX`-discard dots both count as served transfer dots and may advance the pre-visible `current X`, but a true post-priming BG FIFO starvation dot must not advance that `current X`; instead it should explicitly stretch `Mode 3` by one dot and leave the transfer-side `current X` in place until pixel service resumes.
- In the same current baseline, the transfer side should keep one explicit internal phase boundary between the hidden pre-visible startup dots, the follow-on `SCX` discard path, and ordinary visible pixel output. Keep `current X` derivation phase-owned instead of inferring it ad hoc from whichever counters happen to be nonzero at the call site; that leaves one explicit seam for the later stricter docboy-style `LX` rewrite without having to rediscover which dots belong to startup, `SCX` discard, or visible output.
- In the same current baseline, one explicit `current_transfer_x`-style counter should own Mode `3` dot ownership for BG/window/OBJ arbitration instead of rebuilding that ownership from separate pre-visible and visible-output counters at each call site. It is acceptable for the current repo baseline to keep that counter behaviorally equivalent to the old heuristic phases while the stricter docboy-style `LX` rewrite is still pending, but the ownership source itself should already be centralized so later timing changes do not require another cross-file search for hidden `current X` derivations.
- Keep the stricter "BG FIFO push only when empty" rule coupled to the real fetcher / BG-OBJ interruption model. Do not land that push-side tightening in isolation if the current shared fetcher state cannot yet preserve already-validated DMG cases such as `dmg-acid2`; otherwise the project risks regressing known-good framebuffer output while chasing a more correct internal contract.
- The same caution applies to moving OBJ-start arbitration wholesale onto the BG fetcher's `Push` state. A March 26, 2026 attempt to combine strict FIFO-empty BG push with `Push`-anchored OBJ start regressed repo-managed external DMG cases including `mooneye acceptance/ppu/hblank_ly_scx_timing-GS`, `intr_2_mode0_timing`, `intr_2_oam_ok_timing`, `manual-only/sprite_priority`, `hacktix/strikethrough`, and `mealybug m3_bgp_change`, so that behavior remains explicitly deferred until the shared BG/window/OBJ fetcher model is rewritten more completely.
- Treat Mode 2 as a preparatory pipeline phase for Mode 3, not as an isolated bookkeeping pass.
- The list of visible sprites produced in Mode 2 should feed directly into Mode 3 object timing and mixing logic.
- A shape such as `SelectedSpritesForLine`, `ObjectFetcherState`, `OamFifo`, `ObjectPixel`, `SpritePriorityResolver`, and `BgObjMixer` is a good fit for keeping sprite work explicit and testable.
- A shape such as `wy_triggered`, `window_active_this_line`, `window_line_counter`, `window_x_counter`, explicit window-start events, and pending WX/LCDC-related glitch state is a good fit for keeping window behavior explicit and testable.
- A `StatController`-style unit inside the PPU is a good fit for owning `STAT` enable bits, live coincidence calculation, internal STAT-line composition, rising-edge detection, and the DMG `STAT` write quirk.
- A shape such as `current_oam_scan_row`, `OamCorruptionController`, and `OamCorruptionEventKind` is a good fit for keeping the DMG-family OAM corruption bug deterministic and testable.
- A shape such as `lcd_enabled`, `panel_blank_forced`, `blank_frame_pending`, an explicit raster-start state for LCD re-enable, and a clean pixel-pipeline reset path is a good fit for making LCD power transitions reproducible and testable.
- A fetcher-source distinction such as `BackgroundFetch` versus `WindowFetch` is preferred over late coordinate branching at mix time.
- Use one consistent local term for the sprite-pixel queue; `OBJ FIFO`, `OAM FIFO`, and `ObjectFifo` in this documentation refer to the same hardware-facing queue.
- The object FIFO should carry per-pixel metadata such as color index, palette selection, OBJ priority attribute, X-priority information, OAM-order tie-break information, and transparency.
- Keep OBJ/OBJ priority resolution separate from BG/OBJ mixing so the BG-over-OBJ attribute is applied only after the winning sprite pixel has been chosen.
- Apply X flip, Y flip, palette selection, and `8x16` tile-row mapping during object fetch and FIFO population rather than as a framebuffer post-process.
- The BG-to-window fetch transition should be represented as an explicit pipeline event rather than as a late conditional in the pixel mixer.
- Let the main PPU scheduler remain the source of truth for live mode and `LY`; the `STAT` controller should consume that state rather than re-derive timing independently.
- LCD STAT interrupt requests should leave the PPU through the shared interrupt-controller path rather than by mutating CPU state directly.
- STAT mode transitions should be modeled from the real dot schedule, not reconstructed after the scanline.
- Document and preserve the DMG-specific STAT write quirk when STAT behavior is implemented in detail; do not assume GBC-in-DMG-mode behaves identically.
- Mid-frame writes to LCD-visible registers should be interpreted on the same dot timeline that drives mode, fetcher, FIFO, and interrupt behavior.
- Keep the visible panel-blanking policy separate from the internal pixel pipeline so the first post-enable blank frame does not accidentally become a delayed scheduler start.
- A `SkipBoot` path should synthesize internal LCD mode, dot position, and any relevant pipeline state coherently with the visible post-boot register snapshot instead of inventing a contradictory hidden phase.
- In the current Phase `4.1` spine, keep `LY` plus per-line dot position as the raster source of truth and allow only one explicit startup-mode latch when `SkipBoot` needs to preserve the published post-boot `STAT` mode at handoff before the standard raster baseline takes over on the next dot.
- Do not present `OBP0` and `OBP1` as stable fixed post-boot values in DMG-family direct-boot presets; those registers should remain under an explicit uninitialized-state policy when firmware execution is skipped.
- Let the PPU define when VRAM/OAM are logically inaccessible, while the bus remains responsible for exposing the observable blocked-access result to other actors.
- Keep the PPU as the source of truth for whether VRAM or OAM are currently accessible, but let the bus enforce the resulting CPU-visible read/write behavior.
- Let the PPU raise LCD interrupt requests through the shared interrupt-controller path rather than owning `IF` state or dispatching CPU interrupt service directly.
- Let the PPU own the current Mode `2` OAM row and the corruption formulas, while the bus and CPU only feed address-based and IDU-based trigger events into that controller.

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
- treating LCD STAT interrupts as level-triggered "condition is true" events instead of rising edges on one shared internal line
- recalculating LCD STAT source timing independently from the real PPU mode scheduler
- desynchronizing the mode exposed through `STAT` from the mode the bus uses for VRAM/OAM blocking
- treating `LCDC.7` as a cosmetic display-visibility bit instead of a master LCD/PPU power transition
- pausing the whole machine when the LCD is disabled instead of only disabling the LCD/PPU path
- delaying LCD on/off writes until the end of a scanline or frame without hardware evidence
- resuming fetchers, FIFOs, or partial scanlines after LCD re-enable instead of restarting from a clean raster state
- modeling the first post-enable blank frame as a delayed PPU restart instead of as panel-visible blanking over an already-running internal pipeline
- resolving BG/OBJ mixing before resolving which OBJ pixel actually wins an overlap
- using X visibility as part of Mode 2 sprite selection and thereby hiding real `10`-sprite-per-line exhaustion
- treating OBJ color `0` as white output instead of transparency
- collapsing sprite timing into a constant per-sprite penalty with no explicit object-fetch state
- treating STAT behavior as a generic interrupt source without hardware-specific LCD quirks
- storing `LY` as a writable register instead of exposing the live scanline
- letting LCD-visible MMIO writes bypass the temporal PPU model and only affect a later renderer pass
- synthesizing `SkipBoot` LCD registers without a matching hidden PPU phase
- modeling OAM corruption as an opcode blacklist instead of as Mode `2` plus micro-event hardware behavior
- treating all blocked OAM access the same and thereby triggering OAM corruption during Mode `3`
- forgetting that the first OAM row is special and should remain immune to the basic corruption patterns

## Open questions

- the exact level of sprite-fetch and window-trigger detail required for the first DMG milestone
