# PPU

## Scope

Own LCD/PPU mode progression, rendering state, VRAM/OAM access rules, STAT behavior, fetcher/FIFO behavior, and frame output generation.

## Hardware model

Model PPU modes explicitly. Separate fetcher/FIFO logic when it improves clarity and timing fidelity.

Even in DMG-only work, avoid hard-wiring the design to a single permanent VRAM interpretation or a renderer that only understands four grayscale outputs.
For this project, the PPU should be modeled dot-by-dot, where `1 dot = 1 T-cycle`.

## Evidence policy

- Treat Pan Docs plus hardware-backed timing research as the default source of truth for the external PPU contract.
- Use `[hardware fact]` for rules backed by documentation, hardware research, or strong oracle closure, and `[inference]` for design guidance that is strongly suggested but not fully closed.
- [PPU.md](./PPU.md) is the authoritative hardware handbook. Repo-local migration constraints and compatibility notes live in [PPU-REIMPLEMENTATION.md](./PPU-REIMPLEMENTATION.md), which never overrides this file.

## Normative hardware contract

The sections from `Responsibilities` through `Tests` define the hardware-facing contract for a new implementation.

Use those sections first when designing or reimplementing the PPU.
Consult [PPU-REIMPLEMENTATION.md](./PPU-REIMPLEMENTATION.md) only when you need to preserve current repo behavior or stage a migration without reopening already-closed tests.

## Responsibilities

- mode transitions, scanline progression, and LCD on/off raster restart behavior
- panel-visible output state, including LCD-off blanking and the first blank frame after LCD re-enable
- current Mode `2` OAM-row progression and sprite selection
- background, window, and object pixel-pipeline behavior
- pixel priority, transparency, and BG/OBJ mixing rules
- MMIO-visible LCD/PPU register semantics, including separation between MMIO-owned storage and the register view visible to the active pixel pipeline
- `STAT`, `LY`, `LYC`, and LCD-visible interrupt generation
- VRAM/OAM accessibility and the PPU-visible consequences of bus/DMA ownership, without owning DMA scheduling
- DMG-family OAM corruption behavior

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
- Keep the retained Mode `2` DMA-blocked `Y/X` fallback separate from any late `Mode 3` OBJ metadata word captured for conflict handling; late tile/attribute reads must not overwrite the coarse scan-time `Y/X` state.
- Future HBlank-conditioned transfers should use the PPU's live mode or HBlank-visible state as an input to DMA advance conditions without moving HDMA scheduling logic into the PPU or the bus.

## LCD MMIO contract baseline

- `LCDC` should remain owned by the PPU/LCD controller rather than by a generic MMIO byte bank.
- Writing `LCDC.7` should trigger immediate LCD/PPU side effects, including the LCD enable/disable transition and the corresponding VRAM/OAM accessibility rules.
- `STAT` should be modeled as a mixed register with writable interrupt-enable fields and dynamic read-only fields for coincidence and the current PPU mode.
- Preserve the documented DMG-specific spurious `STAT` interrupt quirk on `STAT` writes; do not assume the same write behavior on GBC running in DMG mode.
- `LY` should be read-only and reflect the live scanline `0-153`; writes must not behave like storage updates.
- `LYC` is readable and writable storage, but its comparison effect belongs to the live PPU state and should be evaluated continuously against `LY`.
- `SCX`, `SCY`, `WX`, and `WY` should be modeled as MMIO-visible PPU registers whose mid-frame writes participate in the same temporal PPU model rather than a deferred renderer recomputation.
- `BGP`, `OBP0`, and `OBP1` should remain PPU-owned DMG palette registers.
- On the shared scheduler path, CPU-originated writes to PPU MMIO registers should stage during the CPU micro-operation phase and commit on the same T-cycle during a dedicated MMIO-commit phase.
- Keep MMIO-owned storage separate from the register view currently visible to the active pixel pipeline.
- That active-pipeline-visible register view should drive Mode `3` BG/window/object fetch decisions, BG/OBJ palette lookup, and other in-flight pipeline reads of `LCDC`, `SCX`, `SCY`, `WX`, `WY`, `BGP`, and `OBP*`.
- The design should also leave room for a previous-dot or pipeline-visible snapshot where live-write-sensitive DMG behavior needs it, especially for window activation, tile-data selection, and palette-conflict handling.
- Do not import the CGB-specific `signed -> unsigned` "reuse the last unsigned fetch byte" behavior into the DMG baseline. If that CGB-family glitch is modeled later, keep it explicitly model-gated in the future CGB path instead of treating it as a generic DMG fetcher rule.
- For `OBP0` and `OBP1`, the low two bits must not change the meaning of OBJ color index `0`, because that index remains transparent.
- On DMG-family hardware, writes to `BGP`, `OBP0`, and `OBP1` during Mode `3` should not be treated as ordinary "new value is visible only from the next pixel onward" MMIO updates. The PPU design should leave room for documented palette-conflict artifacts, including transient write values, limited retroactive recoloring, and the observed early-HBlank tail where such conflicts may still remain panel-visible.
- Keep the DMG BG palette-output model split from the raw current-scanline color pipeline. The CPU-path `BGP` model should keep three behaviors explicit and separate: delayed pipeline-visible writes, a narrow previous-line boundary repaint seam fed only by that delayed class, and retroactive panel recolor when either the first visible-line write lands at `visible_pixels_output == 0` / `current_transfer_x == 0` with no selected sprites or the already-visible BG tail is entirely color `0`.
- Keep the sprite-coupled DMG `BGP` live-write follow-up explicit too: a single left sprite can shift the first two CPU-path write onsets by sprite phase and can expose a short transient left-edge range on the second write before the final palette wins.
- Keep BG and OBJ palette-conflict handling separable; do not assume `BGP` and `OBP*` share the same retroactive span or the same conflict window over already-mixed pixels.

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
- While the LCD is disabled, mode-dependent LCD STAT sources should not continue to fire as if the raster were still advancing invisibly, and `LY` should not keep advancing accidentally just because a generic line counter happened to keep ticking.
- If the project later offers a debug warning for disabling the LCD outside VBlank, that warning must remain observational only and must not change the emulated hardware result.

## LCD-visible output baseline

- The project should distinguish between the internal PPU pixel pipeline and the visible LCD-panel output state.
- When the LCD is disabled, the visible output should be forced to the LCD-off DMG white state rather than to palette color `0` as if the PPU were still presenting ordinary pixels.
- After re-enabling the LCD, the PPU may resume internal drawing immediately while the panel-visible output remains forced blank for the first full frame.
- The "first full frame stays blank" rule should be modeled as visible-output behavior, not as a delayed start of the internal PPU scheduler.

## STAT register baseline

- `STAT` should remain a mixed register whose writable portion is the software-configured interrupt-enable mask and whose read-only portion is derived from live PPU state.
- Treat `STAT` bit `7` as non-writable and keep its readback policy explicit and model-gated rather than baking in one universal DMG invariant here. In this handbook, software does not own bit `7`, while bits `6-0` carry the documented interrupt-enable, coincidence, and mode semantics.
- Bits `6-3` should be treated as writable enables for the `LYC==LY`, Mode `2`, Mode `1`, and Mode `0` STAT sources.
- Bit `2` should expose the live `LYC==LY` coincidence state as a read-only flag.
- Bits `1-0` should expose the live current PPU mode as a read-only value: `0` HBlank, `1` VBlank, `2` OAM scan, `3` drawing.
- When the LCD/PPU is disabled through `LCDC.7 = 0`, `STAT` mode bits should read back as `0`.
- Writes to `STAT` must not overwrite the live mode bits or the coincidence flag.

## LY / LYC coincidence baseline

- [hardware fact] `LY` should advance through the live scanline range `0..=153`, including `144..=153` during VBlank.
- [inference] On DMG-family timing, the bus-facing `FF44` readback should advance to the next scanline during the last machine cycle of HBlank before the full raster wrap completes; do not force bus-visible `LY` reads to be identical to the internal raster/comparison line at every dot.
- [hardware fact] The `LYC==LY` flag should come from a continuous comparison between the live `LY` and `LYC` values, not from a once-per-line event cache.
- [hardware fact] Writing `LYC` should immediately reevaluate the live coincidence state rather than waiting for the next scanline boundary.
- [inference] For a from-scratch implementation, do not treat LCD-off readback as "retain the last active-LCD coincidence result". While the LCD is disabled, the external `LY` / `STAT` contract should follow the LCD-off readback rules explicitly instead of pretending the active raster comparison is still live.
- [hardware fact] Coincidence should remain possible during VBlank as well as during visible scanlines.
- [hardware fact] The PPU should not model `LYC` as "schedule a future interrupt when LY reaches this line"; it is a live comparison input to `STAT`.

## STAT interrupt-line baseline

- The PPU should keep an explicit internal `stat_irq_line` or equivalent signal distinct from the visible `STAT` byte readback.
- That internal line should be computed as the OR of the enabled live sources:
  - `stat_mode0_enable && mode == 0`
  - `stat_mode1_enable && mode == 1`
  - `stat_mode2_enable && mode == 2`
  - `stat_lyc_enable && ly == lyc`
- The Mode `2` STAT path should be modeled carefully around the line-`144` VBlank-entry boundary instead of being flattened into a coarse "Mode 2 stays active through all of VBlank" rule.
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
- CPU MMIO side effects that commit after the earlier PPU dot tick of a T-cycle should still reach the owning PPU state before same-cycle interrupt aggregation.

## STAT write quirk baseline

- On DMG-family hardware, writing `STAT` during Mode `0`, Mode `1`, Mode `2`, or while `LYC==LY` is true should support the documented spurious LCD STAT interrupt behavior.
- That quirk should be modeled as a temporary elevation-equivalent effect on the internal STAT interrupt line rather than as "every write to `STAT` requests an interrupt."
- The quirk must not trigger from a Mode `3` write path merely because `STAT` was written.
- Future GBC-in-DMG-mode support must keep this quirk model-gated rather than inheriting the DMG behavior accidentally.

## LCD re-enable and raster restart baseline

- Re-enabling the LCD should enter one explicit, reproducible raster-start state rather than resuming from an ambiguous saved dot or half-finished scanline.
- The implementation should keep one source of truth for the initial scanline, dot, mode, and related scheduler state used after `LCDC.7: 0 -> 1`.
- If the chosen DMG-family model exposes a short initial Mode `0` readback window immediately after LCD re-enable, keep that window explicit and tested rather than scattering it across special-case guards.
- Re-enabling LCD should restart the PPU timing state through the real scheduler path and rebuild live coincidence plus internal STAT-line state from that raster restart instead of reusing stale disabled-state values.
- The first-full-frame blank period should be counted from that re-enabled raster start, not from the earlier disable event.
- The implementation should also keep one explicit, tested policy for how `LY` behaves while the LCD is disabled and how it re-enters the active raster model after re-enable.

## LCD pipeline reset baseline

- Disabling the LCD should explicitly invalidate or reset in-flight pixel-pipeline state rather than freezing and later resuming a half-consumed scanline.
- That reset should cover at least BG FIFO state, OBJ FIFO state, background/window fetcher state, object-fetch state, window latch/counter state, and any in-progress pixel-mixing state.
- Resetting the LCD pipeline on `LCDC.7` transitions must not discard interrupt requests that were already raised earlier in the same shared T-cycle; those requests belong to the later scheduler interrupt-aggregation step, not to the in-flight raster pipeline state.
- Re-enabling the LCD should start pixel production from a clean pipeline state compatible with the chosen raster-start state.
- Do not resume fetchers or FIFOs from the last active-LCD dot before disable; that would contradict the hardware-facing model of the PPU being off and then starting a new draw again.

## Sprite pipeline baseline

- DMG sprites must be integrated into the real Mode 3 pixel pipeline rather than rendered by a scanline-level compositor layered over a finished background image.
- The PPU should keep separate BG FIFO and OBJ FIFO state and perform BG/OBJ mixing when pixels are popped for LCD output, not after the background has already been rendered.
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
- The selected-sprite list for the line should preserve OAM discovery order for later priority and timing work.
- On DMG-family hardware during active OAM DMA, blocked Mode `2` OAM reads should reuse the last latched `Y/X` word instead of inventing fresh scan-time coordinates or force-clearing selection.
- That coarse Mode `2` DMA-blocked `Y/X` fallback must remain separate from late `Mode 3` sprite-metadata conflicts. Late tile/attribute reads may observe a different same-cycle OAM word, but they must not overwrite the retained Mode `2` `Y/X` latch used by later DMA-blocked selection.

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
- In DMG mode with `LCDC.0 = 0`, visible transfer dots should still consume the BG/window FIFO and advance the same `Mode 3` pipeline timing; only the presented BG/window color is forced to white.
- In DMG mode, `LCDC.0` BG/window gating during pixel output should follow the live visible register copy even after the first visible pixel; this bit does not share the later-pixel delayed-copy rule used by `LCDC.1` OBJ enable.
- A BG pixel of color `0` should not block an eligible OBJ pixel as if the BG were opaque.

## Object fetch and stall baseline

- [hardware fact] Mode 3 should include an explicit object-fetch path instead of treating sprite timing as a scalar penalty.
- [hardware fact] Object fetch should be able to wait for the BG fetcher to reach the relevant point before the sprite data is incorporated into the object FIFO.
- [hardware fact] In the canonical Pan Docs fetcher/FIFO contract, the OBJ fetch path may only seize the shared pixel-slice fetcher when the BG/window fetcher is at `Push` and the BG FIFO is not empty. If the BG FIFO is empty, BG/window refill takes priority and the OBJ fetch waits for the next eligible `Push` opportunity.
- [hardware fact] Sprite fetch work should be able to stall pixel output and lengthen Mode 3 on the shared dot timeline.
- [inference] Mode `3` should expose one explicit per-dot arbitration boundary for "BG may serve this dot" versus "OBJ may start on this dot", and that arbitration should stay shared between output-side BG service and push-side BG/window/OBJ handoff paths instead of being rebuilt by separate ad hoc checks.
- [inference] If startup, left-edge, or hidden-dot behavior needs finer closure, keep transfer-lane ownership, transfer readiness, startup window, and effective BG FIFO occupancy as separate explicit state instead of recovering them from raw `line_dot` or generic FIFO length checks.
- [inference] Keep cached background slices explicit across `Push -> fill -> FIFO` so localized live-write timing can stay narrow and testable.
- [inference] Keep the current transfer-dot execution result explicit enough to answer whether the dot consumed discard, hidden transfer, visible transfer, or a stalled startup/OBJ boundary.
- Detailed repo-local closure for startup, cached-slice live writes, strict push behavior, and left-edge arbitration lives in [PPU-REIMPLEMENTATION.md](./PPU-REIMPLEMENTATION.md).
- [inference] The special DMG timing penalty involving `SCX & 7 > 0` together with a sprite at `X = 0` should have an explicit path in the design even if the exact timing remains documented as partially unsettled.
- [inference] Avoid reducing sprite timing to "add N dots per sprite" without internal fetcher state.
- [inference] BG/window/object fetch helpers should consume `VramBusView` / `OamBusView`-style domain clients rather than unrelated `&[u8]` slices.
- [hardware fact] Late Mode `3` sprite metadata reads should come from live OAM rather than from a frozen Mode `2` metadata snapshot.
- [hardware fact] During DMG OAM DMA, that late metadata path should be able to read the DMA destination word being written on the current cycle instead of the nominal sprite metadata address, because tests such as `hacktix/strikethrough` depend on that conflict window.
- [inference] If that DMA conflict is modeled in detail, keep the current DMA destination address and current DMA byte explicit for the cycle instead of relying on an address-only hint.

## Mid-frame toggle and size-change baseline

- `LCDC.1` should be observable by the sprite pipeline mid-frame, including during an in-flight object fetch.
- If `LCDC.1` is turned off during active object fetching, the design should support an explicit fetch-cancel path with real timing cost rather than a pure visibility flag change.
- `LCDC.2` sprite size should be treated as live state, not as a once-per-frame configuration snapshot.
- In `8x16` mode, line selection and tile-row calculation should treat the sprite as two stacked tiles with even/odd tile pairing derived from the masked tile index.
- If a live `LCDC.2` size change shrinks a previously selected sprite so that the current scanline row falls outside the new height, keep that out-of-range case explicit instead of letting row arithmetic underflow.
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
- Treat the window trigger itself as one explicit activation dot before the restarted window fetcher begins advancing again.
- Window visibility must depend on the combined state of `LCDC.5`, the WY latch, and the runtime WX trigger rather than on a single late visibility flag.
- Window Y addressing must come from a dedicated internal window line counter rather than from naively using `LY - WY` at all times.

## Window coordinate baseline

- `WX` should be interpreted using the hardware `X + 7` convention rather than as a direct screen X coordinate.
- `WY` should be treated as the visible starting scanline for window activation, not as a generic continuously applied Y offset.
- The window should only be considered potentially visible when `WX` is within `0..=166` and `WY` is within `0..=143`.
- `WX = 0` and `WX = 166` should remain explicit edge-case paths because they have distinct DMG-visible glitches.

## Window activation baseline

- [hardware fact] Keep two distinct window-eligibility states explicit: a frame-scoped `WY` latch and the line-local runtime `WX` trigger.
- [hardware fact] At the start of Mode `2` on each scanline, compare `LY` against `WY`; if they match, set the frame-scoped `WY` latch.
- [inference] Reset that frame-scoped `WY` latch during VBlank / frame restart rather than recomputing a faux active state from current `LY` later in the frame.
- [hardware fact] Once that frame-scoped `WY` latch has become true, do not require `LY == WY` again on later scanlines for the window to remain eligible in the frame.
- [hardware fact] The WX condition should be evaluated during pixel production using the current render position of the pipeline.
- [hardware fact] For any given scanline, the window should start only when the frame-scoped `WY` latch is already active, the WX trigger point is reached, and `LCDC.5 = 1`.
- [hardware fact] In DMG mode, `LCDC.0 = 0` should suppress window rendering even if `LCDC.5 = 1`.

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
- Once the window fetcher is already active, turning `LCDC.5` off should not cut away the in-flight window tile immediately.
- That disable should take effect at the end of the current window tile, after which background fetch resumes on a tile boundary from explicit saved BG-side progress.

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
- If `LCDC.5` is disabled during Mode `3` and then re-enabled later on the same scanline, do not model that as a generic "resume window where it left off" path. Keep the same-line reactivation explicitly gated on a new not-yet-served `WX` trigger, and keep room for the documented DMG behavior where the window may restart on the next window row rather than on the interrupted row.
- These glitches should live in fetcher/FIFO/pipeline logic rather than as framebuffer post-processing rules.

## Window edge-case baseline

- `WX = 0` should be treated as a DMG-specific special case whose visible stutter depends on `SCX & 7`, not as a normal "starts at X = -7" case.
- `WX = 166` should retain its special behavior of extending across the following scanline rather than being clipped away as an ordinary out-of-range value.
- `WX = 0` and `WX = 166` should each have their own explicit tests and implementation paths.

## Window and sprite interaction baseline

- Window start should change the BG/window pixel stream before OBJ-versus-BG mixing for the final LCD pixel.
- Starting the window should not automatically clear the OBJ FIFO; the documented reset is on the BG-side FIFO path.
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
- Treat Mode 3 as a variable phase driven by fetcher/FIFO work and stalls.
- Use the `172-289` dot DMG-family range as a sanity envelope, not as a fixed target to quantize to.
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
- Do not generalize the OAM corruption bug to generic OAM blocking in Mode `3`; the documented bug is a Mode `2` OAM-access and IDU phenomenon.

## Canonical external fetcher contract

- [hardware fact] For the BG/window pixel fetcher, keep the canonical external state machine from Pan Docs `pixel_fifo` explicit in the design and in the documentation: `TileIndex -> TileDataLow -> TileDataHigh -> Sleep -> Push`.
- [hardware fact] Pan Docs `Rendering Internals` also presents a simplified four-step summary (`TileIndex -> TileDataLow -> TileDataHigh -> Push`). For this project's normative from-scratch contract, treat `pixel_fifo` as authoritative for the external fetcher schedule and read the `Rendering Internals` sequence as a higher-level summary, not as a competing timing model.
- [hardware fact] In that canonical BG/window fetcher contract, `TileIndex`, `TileDataLow`, `TileDataHigh`, and `Sleep` each consume `2` dots, while `Push` is retried every dot until the BG FIFO can accept the fetched `8`-pixel slice.
- [hardware fact] In that same canonical contract, BG/window pixels may be pushed only when the BG FIFO is empty; a non-empty BG FIFO keeps the fetcher in `Push` retry until the queue can accept the full `8`-pixel slice.
- [inference] For a new implementation in this project, land this `pixel_fifo` contract first and add finer startup, push, or arbitration seams later.

## Residual uncertainty

- The highest remaining residual uncertainty is concentrated in left-edge/startup seams, fine window restart and live-write timing, and some LCD on/off plus `STAT` boundary timing.
- Keep those seams explicit and localized in the design rather than folding them back into the canonical external fetcher contract above.
- Use [PPU-REIMPLEMENTATION.md](./PPU-REIMPLEMENTATION.md) for repo-local closure status, rollout constraints, and compatibility notes.

## Dependencies

- bus / MMIO wiring and video-memory access views
- CPU
- T-cycle scheduler or clock source
- interrupt controller
- DMA
- model/revision configuration

## Primary references

- Pan Docs PPU/LCD sections, especially the rendering, `pixel_fifo`, LCDC/STAT, VRAM/OAM access, and OAM-corruption pages
- AntonioND cycle-accurate timing material
- Gekkio hardware research and technical reference material where applicable

For fetcher/FIFO behavior and raster-visible glitches, use the references in this order:

1. Pan Docs and hardware research for the canonical external fetcher contract and register semantics
2. Kevtris, AntonioND, Gekkio, and Matt Currie material and test ROMs for DMG timing refinements, window behavior, and raster-visible quirks
3. Open-source emulators only as implementation cross-checks once the primary contract is already fixed

Do not treat docboy or any other emulator as a normative source for DMG fetcher behavior when it conflicts with the primary references above.

## Open-source emulator references

Priority order:

1. SameBoy
2. docboy
3. accurateboy
4. binjgb
5. GameRoy
6. Mooneye GB
7. Gambatte
8. Danger Boy

For PPU work, this order is weighted by usefulness for DMG pixel FIFO, window timing, and raster-visible behavior, not only by aggregate shootout score.

## Tests

For DMG bring-up and PPU refactor closure, use the following finer-grained maturity ladder as the practical test-order guideline:

| order | maturity stage | family | ROM | domain | complexity | PPU ownership | original # |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Base raster / smoke | acid | `dmg-acid2.gb` | PPU | VERY LOW | general Mode 3 raster, BG/WIN/OBJ mixing, left edge / startup | 2 |
| 2 | Visible raster and post-boot state | daid | `ppu_scanline_bgp.gb` | PPU | MEDIUM | per-scanline `BGP`, visible raster | 41 |
| 3 | Visible raster and post-boot state | hacktix | `bully.gb` | PPU | HIGH | visible VRAM / tilemap seed after boot | 139 |
| 4 | Base OAM / bus visibility | mooneye | `acceptance/bits/mem_oam.gb` | PPU / OAM | MEDIUM | OAM bus access, visible blocking / reads | 45 |
| 5 | Base sprites / priority | mooneye | `manual-only/sprite_priority.gb` | PPU | HIGH | OBJ priority, X / OAM order, BG / OBJ mixing | 138 |
| 6 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/intr_1_2_timing-GS.gb` | PPU | HIGH | `STAT`, Mode `1 -> 2` transition | 78 |
| 7 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/intr_2_0_timing.gb` | PPU | HIGH | `STAT`, Mode `2 -> 0` transition | 79 |
| 8 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/intr_2_mode3_timing.gb` | PPU | HIGH | `STAT`, Mode `2 -> 3` transition | 82 |
| 9 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/intr_2_oam_ok_timing.gb` | PPU | HIGH | `STAT` and OAM release at the mode edge | 83 |
| 10 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/vblank_stat_intr-GS.gb` | PPU | HIGH | Mode `1`, VBlank `STAT` IRQ timing | 88 |
| 11 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/stat_irq_blocking.gb` | PPU | HIGH | `STAT` IRQ line blocking, edge detection | 86 |
| 12 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/stat_lyc_onoff.gb` | PPU | HIGH | `LYC == LY`, LCD off/on, `STAT` | 87 |
| 13 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/hblank_ly_scx_timing-GS.gb` | PPU | VERY HIGH | late Mode `0` / HBlank, `LY` / `SCX` seam | 77 |
| 14 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/intr_2_mode0_timing.gb` | PPU | VERY HIGH | `STAT` Mode `0` versus variable Mode `3` end | 80 |
| 15 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/intr_2_mode0_timing_sprites.gb` | PPU | VERY HIGH | `STAT` Mode `2 -> 0` with sprite stalls | 81 |
| 16 | LCD off/on and restart | mooneye | `acceptance/ppu/lcdon_timing-GS.gb` | PPU | VERY HIGH | LCD on, raster restart, initial `LY` / `STAT` | 84 |
| 17 | LCD off/on and restart | mooneye | `acceptance/ppu/lcdon_write_timing-GS.gb` | PPU | VERY HIGH | `LCDC.7` write timing, restart, `LY` / `STAT` | 85 |
| 18 | DMG OAM quirks | blargg | `oam_bug/1-lcd_sync.gb` | PPU / OAM | HIGH | Mode `2` OAM corruption, LCD synchrony | 22 |
| 19 | DMG OAM quirks | blargg | `oam_bug/2-causes.gb` | PPU / OAM | HIGH | Mode `2` OAM corruption, valid causes | 23 |
| 20 | DMG OAM quirks | blargg | `oam_bug/3-non_causes.gb` | PPU / OAM | HIGH | Mode `2` OAM corruption, exclusions | 24 |
| 21 | DMG OAM quirks | blargg | `oam_bug/4-scanline_timing.gb` | PPU / OAM | HIGH | Mode `2` OAM corruption, per-scanline timing | 25 |
| 22 | DMG OAM quirks | blargg | `oam_bug/5-timing_bug.gb` | PPU / OAM | VERY HIGH | Mode `2` OAM corruption, bug window | 26 |
| 23 | DMG OAM quirks | blargg | `oam_bug/6-timing_no_bug.gb` | PPU / OAM | VERY HIGH | Mode `2` OAM corruption, non-bug window | 27 |
| 24 | DMG OAM quirks | blargg | `oam_bug/8-instr_effect.gb` | PPU / OAM | VERY HIGH | Mode `2` OAM corruption, CPU-access-dependent effects | 28 |
| 25 | DMA quirks + sprite metadata | hacktix | `strikethrough.gb` | PPU | VERY HIGH | Mode `3` OBJ metadata, OAM DMA conflict | 140 |
| 26 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m2_win_en_toggle.gb` | PPU | VERY HIGH | Mode `2`, window-enable latch, `LCDC.5` | 144 |
| 27 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_bgp_change.gb` | PPU | VERY HIGH | Mode `3`, live `BGP`, palette conflict | 145 |
| 28 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_bgp_change_sprites.gb` | PPU | VERY HIGH | Mode `3`, live `BGP` with OBJ interaction | 146 |
| 29 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_bg_en_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.0` BG enable | 147 |
| 30 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_bg_map_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.3` BG map | 148 |
| 31 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_obj_en_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.1` OBJ enable | 149 |
| 32 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_obj_en_change_variant.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.1` OBJ enable, timing variant | 150 |
| 33 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_obj_size_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.2` OBJ size change | 151 |
| 34 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_obj_size_change_scx.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.2` size change with `SCX` discard | 152 |
| 35 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_tile_sel_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.4` tile-data select | 153 |
| 36 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_tile_sel_win_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.4` with window fetch | 154 |
| 37 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_win_en_change_multiple.gb` | PPU | VERY HIGH | Mode `3`, `LCDC.5` toggles, window restart | 155 |
| 38 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_win_en_change_multiple_wx.gb` | PPU | VERY HIGH | Mode `3`, `LCDC.5` plus `WX` retarget | 156 |
| 39 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_lcdc_win_map_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.6` window map | 157 |
| 40 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_obp0_change.gb` | PPU | VERY HIGH | Mode `3`, live `OBP0`, OBJ palette conflict | 158 |
| 41 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_scx_high_5_bits.gb` | PPU | VERY HIGH | Mode `3`, `SCX` high bits, BG fetch origin | 159 |
| 42 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_scx_low_3_bits.gb` | PPU | VERY HIGH | Mode `3`, `SCX` low bits, pixel discard | 160 |
| 43 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_scy_change.gb` | PPU | VERY HIGH | Mode `3`, live `SCY`, BG row selection | 161 |
| 44 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_window_timing.gb` | PPU | VERY HIGH | Mode `3`, window start, fetcher restart | 162 |
| 45 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_window_timing_wx_0.gb` | PPU | VERY HIGH | Mode `3`, window start with `WX = 0` edge case | 163 |
| 46 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_wx_4_change.gb` | PPU | VERY HIGH | Mode `3`, live `WX`, edge case | 164 |
| 47 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_wx_4_change_sprites.gb` | PPU | VERY HIGH | Mode `3`, live `WX` with OBJ interaction | 165 |
| 48 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_wx_5_change.gb` | PPU | VERY HIGH | Mode `3`, live `WX` timing | 166 |
| 49 | Mode 3 hi-fi / live writes | mealybug-tearoom-tests | `ppu/m3_wx_6_change.gb` | PPU | VERY HIGH | Mode `3`, live `WX` timing | 167 |

Project-owned tests:
- tests for variable Mode 3 timing, `SCX` discard behavior, and sprite-induced stalls
- tests for the canonical BG/window fetcher phase order: `TileIndex -> TileDataLow -> TileDataHigh -> Sleep -> Push`
- tests that BG/window `Push` retries until the BG FIFO is empty and does not push into a non-empty BG FIFO
- tests that OBJ fetch may seize the shared fetcher only from an eligible BG/window `Push` point, with BG refill still taking priority when the BG FIFO is empty
- tests for Mode 2 sprite selection using Y only, including horizontally off-screen sprites still consuming one of the `10` slots
- tests for DMG OBJ/OBJ priority: lower `X` wins, then OAM order on equal `X`
- tests for OBJ color `0` transparency and transparent object FIFO filler behavior
- tests for BG/OBJ mixing using the winning OBJ pixel before applying the BG-over-OBJ rule
- tests for `8x8` versus `8x16` selection and row mapping, including bit `0` ignored on `8x16` tile indices
- tests for top-edge and bottom-edge partial sprite visibility such as `Y = 2` and `Y = 154`
- tests for mid-frame `LCDC.1` and `LCDC.2` changes
- tests for WY latch timing at Mode 2 start and WX-trigger timing during Mode 3
- tests for window fetcher reset and BG FIFO clear when the window starts mid-scanline
- tests for the internal window line counter, including increment-only-when-started and reset during VBlank
- tests for `WX = 0` and `WX = 166` special behavior
- tests for DMG `LCDC.0` suppressing window rendering even when `LCDC.5 = 1`
- tests for mid-frame `WX`, `WY`, and `LCDC.5` writes
- tests for `LCDC.5` disable during active window fetch and same-scanline re-enable with `WX` retargeting
- tests for window-start and window-glitch cases that continue into later BG/OBJ mixing without resetting the OBJ FIFO incorrectly
- tests for live `STAT` readback composition: documented writable enable bits, live mode/coincidence bits, and the chosen bit-`7` model
- tests for `LY` covering `0..=153`, including `LYC` matches at `144`, `153`, and the `153 -> 0` wrap
- tests for immediate `LYC` write reevaluation of `STAT.2` and the internal STAT interrupt line
- tests for each enabled LCD STAT mode source path for Mode `0`, Mode `1`, and Mode `2`
- tests for the line-start Mode `2` / LCD STAT chronology used by raster-effect ROMs, including the non-line-`0` pretrigger path and first-line timing differences when a handler writes back into PPU MMIO during the same scanline
- tests for LCD STAT rising-edge behavior and STAT blocking across consecutive enabled sources such as Mode `0` followed by Mode `1`
- tests that Mode `3` never acts as a direct LCD STAT interrupt source
- tests that entering Mode `1` can request both VBlank interrupt and LCD STAT interrupt independently
- tests for DMG-family `STAT` write quirk in Mode `2`, Mode `0`, Mode `1`, and coincidence-active cases, plus a negative test for Mode `3`
- tests that the mode reported through `STAT` matches the same live state used by the bus to block or allow VRAM/OAM access
- tests for blocked CPU VRAM/OAM access semantics, including ignored writes and blocked-read return values in the relevant modes
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

## Known pitfalls

Fetcher / FIFO / timing:
- modeling the PPU as a scanline renderer instead of a dot-by-dot pipeline
- modeling Mode `2` as an instant scan instead of a fixed `80`-dot phase
- assuming Mode `3` starts with valid queued pixels and no pipeline startup cost
- forcing Mode `3` or HBlank to a constant duration instead of deriving them from fetcher/FIFO work
- implementing BG/window rendering without the canonical fetcher/FIFO model, including `Sleep` and `Push` retry
- allowing BG/window `Push` to ignore BG FIFO occupancy instead of waiting for an eligible push point
- collapsing sprite timing into a constant per-sprite penalty with no explicit object-fetch state
- starting OBJ fetch from arbitrary dots instead of the shared BG/window fetcher arbitration seam

OBJ / mixing:
- resolving BG/OBJ mixing before resolving which OBJ pixel actually wins an overlap
- treating OBJ color `0` as white output instead of transparency
- selecting sprites using X visibility or without respecting OAM order and the per-line limit of `10`

Window:
- modeling the window as a second background layer composited after the scanline instead of as a fetcher/FIFO transition
- deriving the window row solely from `LY - WY` or requiring `LY == WY` again after the WY latch has already fired
- resetting the whole scanline, or clearing the OBJ FIFO, when the window starts instead of resetting only the BG-side fetcher/FIFO state
- ignoring `WX = 0`, `WX = 166`, or mid-frame `WX`/`WY`/`LCDC.5` writes because they are rare edge cases

STAT / LCD / MMIO:
- treating LCD STAT as level-triggered state instead of one internal line with rising-edge interrupt behavior
- recalculating `STAT` timing independently from the real PPU mode scheduler or desynchronizing readable `STAT` mode from VRAM/OAM blocking
- treating `STAT` as a generic interrupt source and forgetting DMG-specific write quirks
- storing `LY` as a writable register instead of exposing the live scanline and live `LYC` comparison
- treating `LCDC.7` as a cosmetic display-visibility bit instead of a master LCD/PPU power transition
- pausing the whole machine when the LCD is disabled, or resuming partial scanlines/fetchers after re-enable instead of restarting from a clean raster state
- modeling the first post-enable blank frame as a delayed PPU restart instead of as panel-visible blanking over an already-running internal pipeline
- letting LCD-visible MMIO writes bypass the temporal PPU model and affect only a later renderer pass
- conflating MMIO-owned register storage with the register view currently visible to the active pixel pipeline

Bus / DMA / OAM:
- treating blocked VRAM/OAM CPU access as ordinary memory access instead of returning the blocked-access result and ignoring blocked writes
- modeling DMA/PPU access conflicts too loosely, especially late Mode `3` OAM metadata behavior during OAM DMA
- baking DMG-only palette assumptions into the final pixel representation so future CGB expansion becomes a redesign

Boot / startup / model gating:
- synthesizing `SkipBoot` LCD registers without a matching hidden PPU phase
- modeling OAM corruption as an opcode blacklist instead of as Mode `2` plus micro-event hardware behavior
- treating all blocked OAM access the same and thereby triggering OAM corruption during Mode `3`
- forgetting that the first OAM row is special and should remain immune to the basic corruption patterns
