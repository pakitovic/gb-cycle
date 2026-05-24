# PPU

## Scope

Own LCD/PPU mode progression, rendering state, VRAM/OAM access rules, STAT behavior, fetcher/FIFO behavior, and frame output generation.

## Hardware model

Model PPU modes explicitly. Separate fetcher/FIFO logic when it improves clarity and timing fidelity.

Even in DMG-only work, avoid hard-wiring the design to a single permanent VRAM interpretation or a renderer that only understands four grayscale outputs. For this project, the PPU should be modeled dot-by-dot, where `1 dot = 1 T-cycle`.

## Evidence policy

- Treat Pan Docs plus hardware-backed timing research as the default source of truth for the external PPU contract.
- Use `[hardware fact]` for rules backed by documentation, hardware research, or strong oracle closure, and `[inference]` for design guidance that is strongly suggested but not fully closed.
- [PPU.md](./PPU.md) is the authoritative hardware handbook. Repo-local migration constraints and compatibility notes live in [PPU-REIMPLEMENTATION.md](./PPU-REIMPLEMENTATION.md), which never overrides this file.

## Normative hardware contract

The sections from `Responsibilities` through `Tests` define the hardware-facing contract for a new implementation.

Use those sections first when designing or reimplementing the PPU. Consult [PPU-REIMPLEMENTATION.md](./PPU-REIMPLEMENTATION.md) only when you need to preserve current repo behavior or stage a migration without reopening already-closed tests.

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
- `BCPS`/`BCPD` and `OCPS`/`OCPD` should remain PPU-owned CGB palette access registers rather than generic bus storage: each side has its own `64`-byte RGB555 palette RAM, the index registers read back with bit `6` forced high and address bits `0-5`, data reads/writes are blocked while the CPU-visible PPU mode is Mode `3`, and a blocked data write still performs the documented auto-increment when bit `7` is set.
- `OPRI` / `FF6C` should remain a PPU-owned native-CGB MMIO latch rather than generic bus storage: reads return `$FE | bit0`, writes store bit `0`, and the visual OBJ/OBJ priority mode remains the boot-latched PPU mode until hardware-backed evidence justifies treating ordinary post-boot writes as a live visual switch.
- On the shared scheduler path, CPU-originated writes to PPU MMIO registers should stage during the CPU micro-operation phase and commit on the same T-cycle during a dedicated MMIO-commit phase.
- The staged PPU MMIO commit route must still respect model/mode availability before bypassing the generic bus route; native-CGB palette writes may stage through this path, while Non-CGB and CGB-family `GbCompatible` mode must keep CGB-only palette MMIO unavailable.
- Keep MMIO-owned storage separate from the register view currently visible to the active pixel pipeline.
- That active-pipeline-visible register view should drive Mode `3` BG/window/object fetch decisions, BG/OBJ palette lookup, and other in-flight pipeline reads of `LCDC`, `SCX`, `SCY`, `WX`, `WY`, `BGP`, and `OBP*`.
- In native CGB BG/window fetch, the tile-number byte comes from VRAM bank `0` and the corresponding tile-map attribute byte comes from VRAM bank `1` at the same map offset; the fetched slice must keep the raw attribute byte stable for already-fetched pixels, use bit `3` for tile-data bank selection, apply bits `5` and `6` before producing the two-bit logical color index, carry bits `0-2` into native RGB555 palette lookup, and feed bit `7` into final BG/OBJ composition. CGB-family `GbCompatible` and experimental `CgbDmgExt` rendering keep the DMG software contract and therefore ignore native CGB BG/window attribute sideband even though they run on CGB silicon.
- For the current product-level CGB model, a live `LCDC.4` change that lands on the same T-cycle as a BG/window bitplane read follows the coarse CGB-C/pre-D closure seam exposed by `cgb-acid-hell`: the affected low or high bitplane byte is substituted with the current tile-number byte, model-gated to native `OperatingMode::Cgb` rendering until revision-specific CGB D/E set/reset behavior is split out. When that substitution lands on a cached push slice, preserve any independent live-refetch flags already attached to the slice so row/tilemap corrections can still run without losing the affected-byte override.
- In the native CGB OBJ fetch baseline, the tile index and attribute byte come from the existing live Mode `3` OAM metadata read; the fetched OBJ tile data must ignore CPU `VBK` and use the attribute bit `3` VRAM bank, the logical OBJ color index must apply bits `5` and `6` for horizontal and vertical flips, and the winning OBJ FIFO pixel must carry the raw CGB OBJ attribute sideband so bits `0-2` feed native RGB555 palette lookup while bit `7` feeds final BG/OBJ composition. CGB-family `GbCompatible` and experimental `CgbDmgExt` OBJ fetches ignore the native CGB attribute bank/palette/priority sideband and keep DMG-compatible OBJ metadata semantics.
- Native CGB RGB555 output should be a parallel core framebuffer surface, not a replacement for the existing grayscale framebuffer: BG/window pixels use `BCPD` palette RAM selected by latched BG attribute bits `0-2`, OBJ pixels use `OCPD` palette RAM selected by latched OBJ attribute bits `0-2`, and OBJ color index `0` stays transparent before any OBJ palette lookup.
- CGB-family DMG-software RGB555 output should still use CGB palette RAM instead of the DMG grayscale path: ordinary BG/window color indices are first remapped through the visible compatibility `BGP` register and then look up BG palette `0`, OBJ color indices are first remapped through `OBP0` or `OBP1` and then look up OBJ palette `0` or `1`, explicit Mode `3` `BGP`/`OBP*` conflict repaint paths can supply an override palette for already-presented pixels, CGB-only palette data MMIO remains unavailable to compatibility-mode software, and the same adapter applies to both `GbCompatible` and experimental `CgbDmgExt`.
- Native CGB BG/OBJ composition should resolve the winning OBJ pixel before comparing against BG/window: BG color index `0` always lets an eligible OBJ win, `LCDC.0 = 0` makes eligible OBJs win regardless of BG/OAM priority bits while keeping BG/window pixels visible when no OBJ wins, and `LCDC.0 = 1` lets BG/window color indices `1-3` win when either BG attribute bit `7` or OAM attribute bit `7` is set.
- The design should also leave room for a previous-dot or pipeline-visible snapshot where live-write-sensitive DMG behavior needs it, especially for window activation, tile-data selection, and palette-conflict handling.
- CGB-family `GbCompatible` and experimental `CgbDmgExt` use the DMG-software `SCX` live-write contract for pre-visible low-bit discard retuning and startup `VisibleTile3` tile-column seams, while native `OperatingMode::Cgb` keeps the native CGB fetch path separate. Mealybug CGB evidence currently requires the early alignment-seed low-bit write to retune the current scanline discard budget, and requires selected startup `VisibleTile3` high-bit writes to preserve the old tile across the whole carried slice rather than importing the later tile-column refetch into those pixels.
- Mode `3` live `SCY` writes should distinguish the BG tilemap row (`(SCY + LY) / 8`) from the tile-data row (`(SCY + LY) % 8`). A write that changes only the tile-data row can retarget pending BG tiledata without rereading the tilemap; a write that crosses a tilemap row can request a full BG tilemap/tiledata refetch while the slice is still in an explicit live-refetch window.
- Live `SCY` handling must preserve independent low-plane and high-plane tiledata provenance when a write lands between the two plane reads. Startup-alignment and early visible-tile seams need explicit latch/retarget state rather than a generic cached-slice recomputation, because hardware-visible pixels can combine old and new row sources within one 8-pixel slice.
- Any startup `SCY` placeholder or retargeted BG pixel generated by those seams must still feed the ordinary BG/OBJ mixer as the BG input for that dot. Do not replace the already-mixed final pixel after object priority has been resolved, or overlapping OBJ pixels will be dropped incorrectly.
- CGB-family `GbCompatible` and experimental `CgbDmgExt` keep a distinct DMG-software `SCY` live-write path: BG tiledata high-plane fetches reuse the low-plane tiledata row when `SCY` changes on the low/high plane tiledata seam, the DMG startup-alignment FIFO latch is not reused wholesale, and the left-sprite startup `VisibleTile2`/`VisibleTile3` row-retarget table remains explicitly CGB-family gated and only armed when the live `SCY` write changes the effective tilemap or tile-data row. This CGB-family path is anchored by `mealybug-tearoom-cgb-extra` `ppu/m3_scy_change.gb`; native `OperatingMode::Cgb` and DMG-family rendering stay separate.
- CGB-family `GbCompatible` and experimental `CgbDmgExt` keep a distinct DMG-software `LCDC.4` live-write startup table for BG tile-data selector changes: it reuses the explicit DMG startup slice override mechanism but does not reuse the monochrome DMG phase table wholesale, and it must not inherit the native CGB same-cycle tile-number substitution. CGB-family SCY row-retarget output remains conditioned on an actual live `SCY` write marker so unrelated `LCDC.4` writes do not consume the SCY retarget table. This path is anchored by `mealybug-tearoom-cgb-extra` `ppu/m3_lcdc_tile_sel_change.gb`.
- CGB-family `GbCompatible` and experimental `CgbDmgExt` use the DMG-software `LCDC.2` live OBJ-size contract for Mode `3` 16-to-8 shrink writes, but keep a separate CGB-family residual phase table instead of reusing the DMG-family plane-selection table wholesale. The CGB-family table applies the live shrink to OBJ fetch bytes, pending FIFO/repaint effects, and per-pixel output override, while preserving observed CGB differences such as fully live `X=16`/`X=33` seams, the write-2 `X=32` low/high-plane split, the shifted `SCX=5..=7` high-half split, and the first-shrink visible-`X=10` `SCX=0` line-start seam. This path is anchored by `mealybug-tearoom-cgb-extra` `ppu/m3_lcdc_obj_size_change.gb` and `ppu/m3_lcdc_obj_size_change_scx.gb`; native `OperatingMode::Cgb` stays out of the DMG palette/OBJ-size compatibility path.
- CGB-family `GbCompatible` and experimental `CgbDmgExt` also keep CGB-specific DMG-software tables for live `LCDC.0` BG-enable and `LCDC.3` BG-map writes: `LCDC.0` uses a distinct single-left-sprite onset table, forced-white BG pixels must stay panel/RGB555 white, restored BG pixels must use the CGB compatibility adapter, and `LCDC.3` clears/retargets startup `VisibleTile2`/`VisibleTile3` differently from monochrome DMG for low sprite phases. These seams are anchored by `mealybug-tearoom-cgb-extra` `ppu/m3_lcdc_bg_en_change.gb` and `ppu/m3_lcdc_bg_map_change.gb`.
- Do not import the CGB-specific `signed -> unsigned` "reuse the last unsigned fetch byte" behavior into the DMG baseline. If that CGB-family glitch is modeled later, keep it explicitly model-gated in the future CGB path instead of treating it as a generic DMG fetcher rule.
- For `OBP0` and `OBP1`, the low two bits must not change the meaning of OBJ color index `0`, because that index remains transparent.
- On DMG-family hardware, writes to `BGP`, `OBP0`, and `OBP1` during Mode `3` should not be treated as ordinary "new value is visible only from the next pixel onward" MMIO updates. The PPU design should leave room for documented palette-conflict artifacts, including transient write values, limited retroactive recoloring, and the observed early-HBlank tail where such conflicts may still remain panel-visible.
- Keep the DMG BG palette-output model split from the raw current-scanline color pipeline. The CPU-path `BGP` model should keep three behaviors explicit and separate: delayed pipeline-visible writes, a narrow previous-line boundary repaint seam fed only by that delayed class, and retroactive panel recolor when either the first visible-line write lands at `visible_pixels_output == 0` / `current_transfer_x == 0` with no selected sprites or the already-visible BG tail is entirely color `0`. When a BG dot was already presented as `LCDC.0`-forced white, that retroactive recolor must leave the panel dot white instead of remapping it through `BGP`.
- CGB-family `GbCompatible` and experimental `CgbDmgExt` use the same DMG-software `BGP`/`OBP*` live-write contract for Mode `3` current-line timing, but their already-presented pixels must be recolored through the CGB compatibility RGB555 adapter. Do not import the DMG-family generic transient palette `previous_visible | value` into CGB-family DMG-software output; generic CGB compatibility `BGP` CPU commits use the committed value for the transient repaint, native CGB continues to skip DMG palette-conflict paths, and the DMG-family previous-scanline boundary repaint seam remains DMG-only.
- Keep the sprite-coupled DMG `BGP` live-write follow-up explicit too: a single left sprite can shift the first two CPU-path write onsets by sprite phase and can expose a short transient left-edge range on the second write before the final palette wins; if that second write lands before the seam window opens, keep the previous palette active until the transient or final onset begins. In CGB-family DMG-software modes, the left-sprite second-write final onset is one pixel earlier for sprite `X=7..13`, uses an explicit `X=14..15` seam at visible `X=11`, and preserves the DMG onset for `X=16..18`.
- The Mealybug `LCDC.1` timing variant uses a late `BGP` black pulse as a visual timing probe after disabling and re-enabling OBJ output; in CGB-family DMG-software modes, the right-edge pulse uses a CGB-specific late-onset table and future-pixel holds count visible BG pixels rather than arbitrary non-visible Mode `3` dots.
- Keep the DMG window-restart `BGP` follow-up explicit as a separate seam from the left-sprite case: the first write can backdate the recent BG tail to a clamped window onset, while the second write may need scanline-position repaint even when recent panel history has stalled. For `WX = 0`, the second-write onset depends on both the internal window tile row and the write arrival point, so model it as a row-dependent cap further limited by the current visible output position instead of as one fixed left-edge threshold.
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
- [inference] On DMG-family timing, the bus-facing `FF44` readback can advance to the next scanline during the last machine cycle of visible-line HBlank before the full raster wrap completes; do not force bus-visible `LY` reads to be identical to the internal raster/comparison line at every dot.
- [inference] Do not apply that early-read seam uniformly through VBlank. For VBlank lines, keep the bus-facing `LY` transition aligned to the explicit line-start update path instead of pre-advancing from the previous line's tail.
- [hardware fact] DMG-family line-`153` timing exposes separate readback and comparison seams: the internal raster line can remain `153` while `FF44` reads as `0` from dot `4`, `LYC=153` compares only across dots `4..8`, and `LYC=0` does not compare until dot `12`.
- [inference] The DMG-family `LYC=0` LCD STAT IRQ source on line `153` has its own dot-`8` pretrigger before readable `STAT.2` rises; CPU MMIO writes to `LYC` or `STAT` that commit on that same scheduler T-cycle must be able to cancel that unaggregated pretrigger before interrupt aggregation.
- [inference] CGB-family line-`153` timing keeps the earlier combined late-`LY0` seam: `FF44` readback and `LYC=0` coincidence switch together at dot `8`, while `LYC=153` remains true from the line start until that dot; do not apply the DMG dot-`4`/dot-`12` split to CGB.
- [inference] The machine-level DMG `SkipBoot` lane keeps the internal synthetic first frame at `LY=0` / `STAT=$85` for scheduler/test determinism and exposes that plain HWIO image to CPU-bus reads; reset-facing boot publication is opt-in through `CustomBoot` or verified `RealBoot` handoff so Mooneye-style boot-HWIO probes are not forced onto the `poweron_*` table.
- [inference] During the first DMG-family reset-facing `CustomBoot` or verified `RealBoot` CPU bus probes, `gbmicrotest` exposes a narrower boot-facing publication table before the general first-frame `FF44` lag: CPU MMIO reads see `LY=0` through delay `119`, `LY=1` through delay `233`, and `LY=2` from delay `234`; this is a CPU-bus readback overlay only, with a `CustomBoot` frame-origin base or a separate `RealBoot` handoff base, not a change to the internal synthetic raster or the verified direct-boot `LY=153` snapshot.
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
- Keep room for the internal LCD STAT interrupt line to lead the readable `STAT.mode` bits by a few T-cycles on DMG-family hardware. Current oracle-backed closure in this repo requires the Mode `0` STAT source to be able to rise up to `4` dots before ordinary HBlank becomes visible through `STAT` readback and before VRAM bus release, but that pretrigger is not universal across the first frame after an LCD re-enable.
- Keep the same distinction for Mode `2` at LCD restart, Mode `0` edges, ordinary line starts, LYC edges, Mode `1` VBlank entry, and the DMG line `143 -> 144` seam: a pretriggered or edge-aligned LCD STAT request may be aggregated in the same T-cycle and wake the CPU, but a CPU instruction reading `IF` earlier in that T-cycle must still see the previously aggregated `IF` value rather than the PPU's unaggregated pending bit.
- [inference] The readable PPU mode and the mode source that feeds the internal STAT IRQ line may need separate DMG-family publication rules at LCD restart and SCX-dependent HBlank seams; do not force one helper to answer both questions if ROM tests distinguish readback, bus release, CPU wake, and same-cycle `IF` visibility.
- [inference] CGB-family compatibility mode also has CPU-visible `STAT.mode` publication seams that are not identical to the internal OBJ-extended Mode `3` tail: Mooneye `acceptance/ppu/intr_2_mode0_timing_sprites.gb` requires a saturated ten-OBJ step-8 FIFO tail starting at OAM `X=4` to publish HBlank once the ready FIFO tail reaches the calibrated early aperture, even if the internal BG/OBJ pipeline still has a pending tail marker.
- [inference] During the first frame after LCD re-enable, current `gbmicrotest` closure requires suppressing the ordinary Mode `0` pretrigger until VBlank, while still allowing specific Mode `0` IRQ-source edges once the restart model reaches the explicit HBlank restore point.
- [inference] On the first line after LCD re-enable, a halted CPU samples the Mode `0` STAT wake at a later `SCX`-aligned aperture than the early `IF` publication edge used by non-HALT code; keep that as CPU wake gating instead of moving the `IF` edge because the `gbmicrotest` HALT and non-HALT HBlank cases distinguish the two paths.
- [inference] DMG Mode `2` STAT has the same kind of split at ordinary OAM starts: the internal source rises at the four-dot pretrigger before the next visible line and same-cycle `IF` reads hide that fresh STAT bit, while the extra HALT wake deferral is limited to the first blank frame after LCD re-enable so steady-state Mooneye-style Mode `2` probes keep their observed counts.
- [inference] At the line `143 -> 144` boundary, DMG Mode `2` STAT is a STAT-only source at the final HBlank pretrigger dot, not a real OAM scan on line `144`; do not let it lock OAM, select sprites, or change readable `STAT.mode` away from VBlank once line `144` begins.
- [inference] The same DMG line `143 -> 144` STAT-only pretrigger still publishes the LCD STAT `IF` edge early enough for gbmicrotest line-`144` probes, but if VBlank is enabled in `IE` the CPU's interrupt service decision is held until line `144` dot `0` so the dedicated VBlank request wins normal priority over LCD STAT.
- [inference] DMG line `153` `LYC=0` STAT has a narrower STAT-only pretrigger: it may request LCD STAT at dot `8` so a running CPU can take the interrupt before the next `DI`, but readable coincidence remains low until dot `12` and a same-dot CPU write that changes `LYC`/`STAT` cancels the unaggregated request; the internal STAT line must stay asserted from the pretrigger through the visible coincidence seam so dot `12` does not generate a second LCD STAT edge.
- [inference] CGB line `153` keeps `LYC=153` as the ordinary line-start edge and uses dot `8` for the `LYC=0` edge; same-cycle `IF` visibility hiding still applies to those CGB edges, but the DMG-only dot-`8` pretrigger/dot-`12` readable split does not.
- [inference] On the first visible line after the line `153 -> 0` VBlank wrap, CPU-visible `STAT.mode` readback lags the Mode `2 -> 3` and Mode `3 -> 0` edges by four dots while the internal raster/bus owner still uses the ordinary line-`0` mode schedule.
- [inference] The DMG `CustomBoot` reset-facing lane also needs a first-frame hidden STAT IRQ phase: visible `LY=0` / `STAT=$85` still begins on the shared machine timeline, but the Mode `0` IRQ source for startup HBlank should use the boot-phase seam rather than the ordinary dot-248 pretrigger, including the observed `SCX&7` boundary exceptions.
- [inference] The DMG-family boot-facing publication overlay keeps its own CPU-visible `STAT` and bus-access table for the early `poweron_*` probes in `CustomBoot` and verified `RealBoot` handoff paths: readable `STAT` reports `$85` for delays `0..=5`, `$84` at `6`, `$86` for `7..=26`, `$87` for `27..=69`, `$84` for `70..=119`, `$80` at `120`, `$82` for `121..=140`, `$83` for `141..=183`, `$80` for `184..=234`, and `$82` at `235`; OAM is CPU-blocked at delays `6..=69`, `120..=183`, and `234..=235`, while VRAM is CPU-blocked at `26..=69` and `140..=183`. Keep this as a boot-gated CPU-bus publication rule instead of reusing ordinary Mode `0` / Mode `2` / Mode `3` helpers.
- [inference] The later-DMG `RealBoot` cartridge handoff keeps a separate handoff-frame Mode `0` seam for startup HBlank tests: ordinary `SCX` values can use the Mode `0` pretrigger, while `SCX&7 == 3` and `SCX&7 == 7` suppress that pretrigger until the visible HBlank boundary; this seam is armed by the real `FF50` handoff and cleared on the next VBlank.
- Entering VBlank at `LY = 144` should be able to request both the dedicated VBlank interrupt and the LCD STAT interrupt for Mode `1` independently when the corresponding `STAT` enable is set.
- [inference] On DMG-family hardware, the dedicated VBlank request at line `144` is also hidden from same-cycle CPU `IF` reads until aggregation completes, which lets a boundary `IF` probe observe a pending STAT bit without also seeing the just-raised VBlank bit in that same T-cycle.
- The same live mode state that feeds `STAT` must also feed VRAM/OAM accessibility decisions so software polling `STAT` sees the same timing the bus uses for blocking.
- On the shared scheduler, the PPU dot tick should happen before current-cycle bus arbitration and interrupt aggregation so `STAT`, LCD IRQ requests, `LY`, and VRAM/OAM restrictions remain coherent for that T-cycle.
- CPU MMIO side effects that commit after the earlier PPU dot tick of a T-cycle should still reach the owning PPU state before same-cycle interrupt aggregation.

## STAT write quirk baseline

- On DMG-family hardware, writing `STAT` during selected Mode `0`, Mode `1`, Mode `2`, or `LYC==LY` windows should support the documented spurious LCD STAT interrupt behavior.
- That quirk should be modeled as a temporary elevation-equivalent effect on the internal STAT interrupt line rather than as "every write to `STAT` requests an interrupt."
- [inference] The write-quirk timing has its own DMG line/dot windows and must not reuse `current_access_mode()` directly: ordinary HBlank writes start at the visible HBlank dot plus `4`, ordinary OAM writes only use the line-start dot, and the frame-start/line-`0` OAM window remains a separate early exception.
- [inference] The DMG write quirk is not limited to writes that clear all STAT enables in VBlank or live coincidence windows; a nonzero write during a quirk-active VBlank window can still request LCD STAT, and that VBlank quirk pulse suppresses the later repeated line-`153` `LYC=0` STAT source so software does not receive a second per-frame STAT interrupt from the same armed line.
- [inference] Nonzero writes that merely arm a future STAT source must not reuse the zero-write OAM/HBlank/restart quirk windows; otherwise an early LCD-restart `STAT=$40` setup write leaves IF STAT pending before the intended line-`1` LYC edge.
- A write that merely arms a source whose raw condition is already active should update the internal line level without synthesizing an immediate ordinary-source request; the next request still requires a later low-to-high edge, which keeps mid-Mode-`2` arming from preempting the next real OAM edge.
- The quirk must not trigger from a Mode `3` write path merely because `STAT` was written.
- Future GBC-in-DMG-mode support must keep this quirk model-gated rather than inheriting the DMG behavior accidentally.

## LCD re-enable and raster restart baseline

- Re-enabling the LCD should enter one explicit, reproducible raster-start state rather than resuming from an ambiguous saved dot or half-finished scanline.
- The implementation should keep one source of truth for the initial scanline, dot, mode, and related scheduler state used after `LCDC.7: 0 -> 1`.
- [inference] On DMG-family LCD restart, CPU-bus `STAT.2` readback on restart line `1` keeps the `LYC==LY` flag suppressed for the first four dots even though the internal coincidence source is already true; keep this as a readback visibility seam rather than moving the internal `LYC` comparison.
- If the chosen DMG-family model exposes a short initial Mode `0` readback window immediately after LCD re-enable, keep that window explicit and tested rather than scattering it across special-case guards.
- [inference] That initial Mode `0` readback window after LCD re-enable should not automatically feed the ordinary Mode `0` STAT IRQ source; current `gbmicrotest` closure treats the first-line Mode `0` IRQ edge as a separate restore-dot event whose offset is grouped from the low three `SCX` bits.
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
- Per-sprite attributes should explicitly include at least BG-over-OBJ priority, X flip, Y flip, DMG palette selection `OBP0` or `OBP1`, and the CGB-only OBJ tile VRAM bank plus OBJ palette index sideband.

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

## CGB OBJ priority baseline

- Keep object selection, OBJ/OBJ drawing priority, and final BG/OBJ composition separate in CGB-family rendering too.
- In native CGB mode, the boot-latched OBJ drawing priority mode prefers the earlier OAM entry for overlapping non-transparent OBJ pixels.
- In CGB compatibility mode, the boot-latched OBJ drawing priority mode prefers the smaller `X` coordinate, with OAM order only breaking same-`X` ties, but this does not enable DMG-family silicon quirks such as OAM corruption.
- Runtime `OPRI` writes update the MMIO latch/readback only in ordinary Phase 10 behavior; the boot-latched visual priority mode remains stable unless a future hardware-backed test proves a live visual switch.

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
- In CGB-family rendering, `LCDC.0` must not force BG/window pixels to white or suppress window activation; it is a BG/window master priority bit for OBJ composition, so BG/window pixels remain visible whenever no eligible OBJ wins the final mix.
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
- CGB-family `GbCompatible` and experimental `CgbDmgExt` keep the DMG-software `LCDC.1` live OBJ-enable contract for visible output, but the single-left-sprite disable onset table is CGB-family specific and must stay separate from both native CGB and monochrome DMG timing.
- `LCDC.2` sprite size should be treated as live state, not as a once-per-frame configuration snapshot.
- CGB-family `GbCompatible` and experimental `CgbDmgExt` must route DMG-software `LCDC.2` 16-to-8 shrink writes through the same explicit active-write, pending-effect, and OBJ-output override machinery as DMG-family rendering, while selecting CGB-specific observed plane seams from the active write phase instead of the DMG-family residual table.
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
- [inference] On CGB-family hardware, treat `LCDC.5` as a line-local window-enable latch sampled during visible-scanline preparation: writes while the scanline is still in Mode `2` must not redecide the current scanline's window activation, while writes once Mode `3` is active can still update the current-line latch for live window start/abort behavior.
- [hardware fact] In DMG mode, `LCDC.0 = 0` should suppress window rendering even if `LCDC.5 = 1`.

## Window tilemap and fetch baseline

- The window tilemap should be selected by `LCDC.6`, independent of the BG tilemap selection.
- In DMG mode, paired live `LCDC.6` writes during window activation can leave explicit old/new tilemap selector seams on the first few activated window tiles; keep that behavior in fetcher/FIFO ownership rather than in framebuffer post-processing.
- On CGB-family hardware running DMG software (`GbCompatible` or `CgbDmgExt`), the paired live `LCDC.6` activation seam has a distinct lead-in before the DMG row-block table: window lines 0-23 keep the second activation tile on the transient map and apply sparse transient/settled masks to the first activation tile, with lines 16-23 observing the second-write latch phase; native CGB mode does not use this DMG-software seam.
- Window tile data addressing should follow `LCDC.4`, matching BG tile addressing rules while remaining separate from OBJ tile handling.
- In DMG mode, paired live `LCDC.4` writes during window fetch can leave per-plane old/new tile-data selector seams on the first few affected window tiles; keep the future-pixel selector override explicit on cached window slices, and keep room for a same-scanline repaint path when the second write lands after some of those window pixels have already been driven.
- On CGB-family hardware running DMG software (`GbCompatible` or `CgbDmgExt`), paired live `LCDC.4` writes during window fetch have a distinct bidirectional seam: signed-to-unsigned writes can affect the current window fetch from line 24 onward while unsigned-to-signed writes can affect the lead-in from line 16 onward, and the signed-to-unsigned fetcher override must be one-shot so later `fetch_x` tiles can still observe the paired unsigned-to-signed write; native CGB mode keeps the native CGB `LCDC.4` timing path instead of this DMG-software seam.
- The fetcher should have explicit BG and window fetch modes rather than reusing BG fetch implicitly through altered coordinates.
- Window tile X should derive from a window-local X counter, not from `SCX`.
- Window tile Y should derive from the internal window line counter, not from `LY + SCY`.
- BG-side `SCX` and `SCY` rereads per tile fetch should remain confined to BG coordinate logic and must not leak into window fetch coordinates.

## Window start-event baseline

- Starting the window should clear the BG FIFO.
- Starting the window should reset the fetcher to its initial fetch step rather than continuing from the current BG fetch phase.
- The window-start event should alter the remaining pixel sequence of the current scanline without replaying or recomputing the whole line.
- The DMG special case `WX = 0 && (SCX & 7) > 0` should be modeled as an explicit path that shortens Mode 3 by `1` dot.
- [inference] The `WX = 0 && (SCX & 7) == 3` terminal seam still keeps CPU-visible `STAT.mode` in Mode `3` for the one-dot preterminal read used by `gbmicrotest` before publishing HBlank on the following probe; do not let the generic terminal-tail early-HBlank rule override that window seam.
- On DMG, there is an explicit low-`WX` seam where turning `LCDC.5` off after the window has already started does not cut away the in-flight window tile immediately; let the current tile complete, then abort back to background from the next fetch boundary.
- That disable should take effect at the end of the current window tile, after which background fetch resumes on a tile boundary from explicit saved BG-side progress.

## Window line-counter baseline

- The PPU should keep an explicit internal window line counter.
- That counter should reset during VBlank.
- The counter should increment only on scanlines where the window actually begins rendering.
- If the same scanline starts the window more than once, advance the internal counter by the number of starts on that line rather than by a flat single step.
- Hiding the window mid-frame via `WX` manipulation or `LCDC.5` should be able to prevent the increment for affected lines.
- Do not define the window row globally as `LY - WY`; that shortcut is not valid for status bars and mid-frame show/hide behavior.

## Window mid-frame write and glitch baseline

- Writes to `WX`, `WY`, and `LCDC.5` during the frame must be visible to the live pipeline rather than deferred until the next frame.
- On CGB-family hardware, keep the Mode `2` and Mode `3` `LCDC.5` effects distinct: Mode `2` writes update MMIO for following scanlines but not the already-prepared line-local window-enable latch, whereas Mode `3` writes update that latch and remain live to the active BG/window fetcher.
- On DMG, before the first visible pixel of the scanline has been emitted, a low-`WX` (`WX < 8`) same-line retarget or re-enable should be allowed to see the current visible `WX` write rather than only the delayed previous-dot pipeline snapshot, so the previsible trigger can start from the matching hidden or previsible transfer dot.
- If the WY latch is already active for the current line and `LCDC.5` was active at line start but is cleared before the WX trigger point, the design should support the documented window-glitch pixel at the would-be window start.
- If `WX` changes after the window has already started on the line and the new trigger position is reached again, the documented bug should be representable as a low-priority color-`0` pixel pushed into the BG FIFO path.
- On CGB-family hardware running DMG software (`GbCompatible` or `CgbDmgExt`), live `WX` writes share the DMG-software MMIO contract but not the full DMG-silicon previsible retarget table: same-line normal restarts after an earlier window start are suppressed, cancel-only low-`WX` aborts preserve the line's window-start count, tile-index-phase previsible `WX` reactivation can insert a raw color-`0` FIFO pixel, and the `WX=4`/`WX=5`/`WX=6` phase repaint remains a bounded CGB-family raw-pixel repaint that a later `WX` restore can cancel until the observed phase guard has passed. The `WX=4` case also exposes plane-source seams: phase `0` combines the current high plane with the delayed window high plane as low-plane data, phase `2` copies the current low plane into the high plane and cancels on the repaint start guard, and phase `6` repaints from delayed window pixels while retaining the trigger guard.
- If `LCDC.5` is disabled during Mode `3` and then re-enabled later on the same scanline, do not model that as a generic "resume window where it left off" path. Keep the same-line reactivation explicitly gated on a new not-yet-served `WX` trigger, and keep room for the documented DMG behavior where the window may restart on the next window row rather than on the interrupted row.
- On DMG, same-line `LCDC.5` re-enable after a missed or aborted window start can expose narrow late-enable seams: allow explicit bounded retroactive repaint of only the affected visible window segment, keyed by the observed onset class, instead of recomputing the whole scanline or resuming the interrupted tile blindly.
- In the DMG low-`WX` disable/re-enable seam, treat the retained left-edge artifact as a full observed prefix span, not just as a tail extension past pixel `8`; when the observed prefix grows beyond `8` pixels, the retroactive repaint may need to repaint the whole retained prefix span.
- In the DMG low-`WX` live-`WX` restart seam, keep the onset-glitch repaint armed for boundary restarts even when there is no visible gap, because the first trigger pixel can still glitch.
- Model the DMG previsible-cancel override as a panel-white contract, not as a hardcoded raw BG color `0`: the override should emit whichever BG raw color maps to panel shade `0` under the current `BGP`.
- When repainting an already-emitted DMG onset-glitch dot, recompute full BG+OBJ mixing and refresh recent panel-dot history for that visible `x`; repainting only the background color is not sufficient once a behind-BG OBJ can become visible.
- When an active window fetch aborts back to background, retarget the in-flight fetch registers immediately onto the resumed BG tile; do not let stale window `tile_index` / `tile_low` / `tile_high` leak into the first resumed BG tile.
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
- Use [PPU-REIMPLEMENTATION.md](./PPU-REIMPLEMENTATION.md) for repo-local guardrails that prevent reopened regressions during internal rewrites.

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

The following DMG ROMs are the PPU no-regression catalog for the already-closed external raster and timing work. Treat the order as a diagnostic grouping, not as an active phase-progress ledger:

| order | diagnostic group | family | ROM | domain | complexity | PPU ownership | original # |
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
| 13 | LCD off/on and restart | mooneye | `acceptance/ppu/lcdon_timing-GS.gb` | PPU | VERY HIGH | LCD on, raster restart, initial `LY` / `STAT` | 84 |
| 14 | LCD off/on and restart | mooneye | `acceptance/ppu/lcdon_write_timing-GS.gb` | PPU | VERY HIGH | `LCDC.7` write timing, restart, `LY` / `STAT` | 85 |
| 15 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/hblank_ly_scx_timing-GS.gb` | PPU | VERY HIGH | late Mode `0` / HBlank, `LY` / `SCX` seam | 77 |
| 16 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/intr_2_mode0_timing.gb` | PPU | VERY HIGH | `STAT` Mode `0` versus variable Mode `3` end | 80 |
| 17 | STAT / LY / LYC / IRQs | mooneye | `acceptance/ppu/intr_2_mode0_timing_sprites.gb` | PPU | VERY HIGH | `STAT` Mode `2 -> 0` with sprite stalls | 81 |
| 18 | DMG OAM quirks | blargg | `oam_bug/1-lcd_sync.gb` | PPU / OAM | HIGH | Mode `2` OAM corruption, LCD synchrony | 22 |
| 19 | DMG OAM quirks | blargg | `oam_bug/2-causes.gb` | PPU / OAM | HIGH | Mode `2` OAM corruption, valid causes | 23 |
| 20 | DMG OAM quirks | blargg | `oam_bug/3-non_causes.gb` | PPU / OAM | HIGH | Mode `2` OAM corruption, exclusions | 24 |
| 21 | DMG OAM quirks | blargg | `oam_bug/4-scanline_timing.gb` | PPU / OAM | HIGH | Mode `2` OAM corruption, per-scanline timing | 25 |
| 22 | DMG OAM quirks | blargg | `oam_bug/5-timing_bug.gb` | PPU / OAM | VERY HIGH | Mode `2` OAM corruption, bug window | 26 |
| 23 | DMG OAM quirks | blargg | `oam_bug/6-timing_no_bug.gb` | PPU / OAM | VERY HIGH | Mode `2` OAM corruption, non-bug window | 27 |
| 24 | DMG OAM quirks | blargg | `oam_bug/8-instr_effect.gb` | PPU / OAM | VERY HIGH | Mode `2` OAM corruption, CPU-access-dependent effects | 28 |
| 25 | DMA quirks + sprite metadata | hacktix | `strikethrough.gb` | PPU | VERY HIGH | Mode `3` OBJ metadata, OAM DMA conflict | 140 |
| 26 | Mode 3 palettes | mealybug-tearoom-tests | `ppu/m2_win_en_toggle.gb` | PPU | VERY HIGH | Mode `2`, window-enable latch, `LCDC.5` | 144 |
| 27 | Mode 3 palettes | mealybug-tearoom-tests | `ppu/m3_bgp_change.gb` | PPU | VERY HIGH | Mode `3`, live `BGP`, palette conflict | 145 |
| 28 | Mode 3 palettes | mealybug-tearoom-tests | `ppu/m3_bgp_change_sprites.gb` | PPU | VERY HIGH | Mode `3`, live `BGP` with OBJ interaction | 146 |
| 29 | Mode 3 palettes | mealybug-tearoom-tests | `ppu/m3_obp0_change.gb` | PPU | VERY HIGH | Mode `3`, live `OBP0`, OBJ palette conflict | 158 |
| 30 | Mode 3 SCX/SCY (FIFO core) | mealybug-tearoom-tests | `ppu/m3_scx_low_3_bits.gb` | PPU | VERY HIGH | Mode `3`, `SCX` low bits, pixel discard | 160 |
| 31 | Mode 3 SCX/SCY (FIFO core) | mealybug-tearoom-tests | `ppu/m3_scx_high_5_bits.gb` | PPU | VERY HIGH | Mode `3`, `SCX` high bits, BG fetch origin | 159 |
| 32 | Mode 3 SCX/SCY (FIFO core) | mealybug-tearoom-tests | `ppu/m3_scy_change.gb` | PPU | VERY HIGH | Mode `3`, live `SCY`, BG row selection | 161 |
| 33 | Mode 3 LCDC BG toggles | mealybug-tearoom-tests | `ppu/m3_lcdc_bg_en_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.0` BG enable | 147 |
| 34 | Mode 3 LCDC BG toggles | mealybug-tearoom-tests | `ppu/m3_lcdc_bg_map_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.3` BG map | 148 |
| 35 | Mode 3 LCDC BG toggles | mealybug-tearoom-tests | `ppu/m3_lcdc_tile_sel_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.4` tile-data select | 153 |
| 36 | Mode 3 LCDC OBJ toggles | mealybug-tearoom-tests | `ppu/m3_lcdc_obj_en_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.1` OBJ enable | 149 |
| 37 | Mode 3 LCDC OBJ toggles | mealybug-tearoom-tests | `ppu/m3_lcdc_obj_en_change_variant.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.1` OBJ enable, timing variant | 150 |
| 38 | Mode 3 LCDC OBJ toggles | mealybug-tearoom-tests | `ppu/m3_lcdc_obj_size_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.2` OBJ size change | 151 |
| 39 | Mode 3 LCDC OBJ toggles | mealybug-tearoom-tests | `ppu/m3_lcdc_obj_size_change_scx.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.2` size change with `SCX` discard | 152 |
| 40 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_window_timing.gb` | PPU | VERY HIGH | Mode `3`, window start, fetcher restart | 162 |
| 41 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_window_timing_wx_0.gb` | PPU | VERY HIGH | Mode `3`, window start with `WX = 0` edge case | 163 |
| 42 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_lcdc_win_map_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.6` window map | 160 |
| 43 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_lcdc_tile_sel_win_change.gb` | PPU | VERY HIGH | Mode `3`, live `LCDC.4` with window fetch | 154 |
| 44 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_lcdc_win_en_change_multiple.gb` | PPU | VERY HIGH | Mode `3`, `LCDC.5` toggles, window restart | 155 |
| 45 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_lcdc_win_en_change_multiple_wx.gb` | PPU | VERY HIGH | Mode `3`, `LCDC.5` plus `WX` retarget | 156 |
| 46 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_wx_4_change.gb` | PPU | VERY HIGH | Mode `3`, live `WX`, edge case | 164 |
| 47 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_wx_5_change.gb` | PPU | VERY HIGH | Mode `3`, live `WX` timing | 166 |
| 48 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_wx_6_change.gb` | PPU | VERY HIGH | Mode `3`, live `WX` timing | 167 |
| 49 | Mode 3 window mechanics | mealybug-tearoom-tests | `ppu/m3_wx_4_change_sprites.gb` | PPU | VERY HIGH | Mode `3`, live `WX` with OBJ interaction | 165 |

Project-owned regression intent:

The external ROM table above is the main no-regression catalog. Repo-owned unit, integration, and synthetic-ROM tests should stay focused on the local invariants that make those ROM outcomes explainable: variable `Mode 3`, BG/window/OBJ fetch arbitration, STAT/LY/LYC chronology, LCD on/off restart behavior, VRAM/OAM blocking, DMG OAM corruption, panel/live-palette seams, and `SkipBoot` continuity.

If one of those areas is rewritten, keep or add a direct project-owned test for the specific invariant instead of relying only on an external framebuffer or serial result. Concrete open project-owned gaps are tracked in [TODO.md](../TODO.md); do not duplicate a historical covered/partial checklist here.

Phase `10` Slice `9` adds direct native-CGB hardening tests for the closure seams that `cgb-acid-hell` can expose visually: `BCPD`/`OCPD` data reads and writes are blocked only during CPU-visible Mode `3` while index writes remain available and blocked auto-increment still advances; CPU VRAM reads/writes remain visible through Mode `2`, are denied during Mode `3`, and recover in HBlank without retaining failed writes; BG/window fetch probes verify bank-`1` attributes, tile-data bank selection, raw attribute sidebands, and CPU `VBK` independence after fetch latching; HDMA/PPU ordering remains tested through the DMA controller with live PPU HBlank/mode input and active-block video-bus conflict visibility. The external closure signal for these internals is `cgb-ppu-hard-cgb-acid-hell`, but any red row must be debugged through these dot-level probes before considering a framebuffer-only visual change.

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
- leaking the boot-facing `CustomBoot` / `RealBoot` PPU publication phase into plain `SkipBoot` boot-HWIO probes
- modeling OAM corruption as an opcode blacklist instead of as Mode `2` plus micro-event hardware behavior
- treating all blocked OAM access the same and thereby triggering OAM corruption during Mode `3`
- forgetting that the first OAM row is special and should remain immune to the basic corruption patterns
