# Phase 4 — Base PPU and visible pipeline

15. **General PPU**
16. **Mode 2 / OAM scan**
17. **Mode 3 / pixel pipeline**
18. **PPU: sprites in detail, priority, BG/OBJ mixing, edge cases**
19. **PPU: window in detail and associated glitches**
20. **PPU: STAT, LY==LYC coincidence, LCD IRQs, and DMG quirks**
21. **PPU: LCD on/off behavior and reactivation**
22. **OAM corruption bug**

#### Goal

Build a truly dot-by-dot PPU, where the visible image emerges from an explicit pipeline based on the fetcher and pixel FIFO.

#### Modules involved

- `ppu/`
- `bus/`
- `scheduler/`
- `dma/`
- `debugger/`

#### Deliverables

##### Base PPU

- general LCD state control
- mode sequencing connected to real timing
- LY and line/dot progression
- direct-boot PPU hidden-state synthesis coherent with the visible post-boot LCD snapshot

##### Mode 2

- OAM scan
- sprite selection for the line
- integration with real temporal restrictions

##### Mode 3

- pixel fetcher
- pixel FIFO
- per-dot pixel production and consumption
- scroll application
- BG, window, and OBJ integration

##### Fine PPU detail

- complete sprite rules
- priorities and mixing
- window and associated glitches
- STAT behavior
- LY==LYC coincidence
- LCD IRQs
- DMG quirks
- LCD on/off and reactivation
- OAM corruption bug

#### Recommended subphase rollout for the current implementation pass

1. `4.1` PPU scheduler spine and explicit state ownership.
   Scope: grow the current register-only PPU baseline into one explicit dot/line/mode state machine with a stable internal source of truth for LCD state, raster position, visible-output state, and direct-boot hidden-state synthesis. If the single-file layout starts obscuring ownership, split `ppu.rs` into focused child modules before timing logic accumulates further.
   Validation: unit tests for MMIO contract preservation, live bus-state derivation, startup-state import, and raster-state reset/snapshot behavior; integration tests that step the shared machine timeline and prove the PPU-visible state stays coherent with scheduler order. Exit criteria: one explicit PPU temporal state model exists, later Mode `2` / `3` work does not need to invent parallel counters, and no scanline-level renderer shortcut is introduced.
2. `4.2` Mode `2` scan and line candidate capture.
   Scope: land live Mode `2` timing, current scanline bookkeeping, and the per-line selected-sprite list driven by `Y`, live `LCDC.2`, OAM order, and the hard `10`-sprite limit.
   Validation: unit tests for sprite selection, `8x8` versus `8x16`, off-screen-`X` still counting, and OAM-order preservation; integration tests for OAM access restriction timing composed with existing DMA behavior. Exit criteria: the current line's sprite candidates are stable explicit state and the Mode `2` schedule is available for later fetch, mixing, and OAM-corruption work.
3. `4.3` BG-only Mode `3` fetcher, FIFO, and visible pixel output.
   Scope: implement the background fetcher, BG FIFO, per-dot pixel production, scroll discard/application, and a deterministic visible-output path without yet layering sprites or window over a finished scanline image.
   Validation: unit tests for fetch-step progression, FIFO fill/pop invariants, and scroll-driven startup behavior; integration tests with synthetic VRAM fixtures that assert visible pixel sequences and Mode `3`-driven VRAM blocking on the shared timeline. Exit criteria: a visible BG-only frame emerges from the real pipeline, Mode `3` is no longer a placeholder duration, and the design keeps a clean seam for later window restarts and OBJ fetch stalls.
4. `4.4` Window activation, fetcher restart, and internal window line counter.
   Scope: add WY latch timing, WX trigger timing, BG FIFO clear plus fetcher restart on window start, the dedicated window line counter, and the first explicit `WX = 0` / `WX = 166` edge paths.
   Validation: unit tests for WY latch semantics, WX trigger timing, and window-line-counter increment rules; integration tests for mid-scanline BG-to-window transition and status-bar style window usage without recomputing the whole line. Exit criteria: BG and window share one pipeline, the window starts as a temporal event rather than a scanline compositor, and later OBJ mixing can consume one BG/window stream instead of two ad hoc renderers.
5. `4.5` OBJ fetch, OBJ FIFO, priority, transparency, and BG/OBJ mixing.
   Scope: add object-fetch stalls, explicit OBJ FIFO state, DMG OBJ/OBJ priority, OBJ transparency, BG-over-OBJ handling, and the key clipping/size cases needed for the base DMG sprite model.
   Validation: unit tests for selection-versus-drawing priority, transparent OBJ color `0`, `8x16` row calculation, and partial top/bottom clipping; integration tests for Mode `3` lengthening, object-fetch cancellation boundaries, and window-plus-sprite interaction on the live pipeline. Exit criteria: sprites participate inside Mode `3` rather than after it, BG/OBJ mixing is resolved per popped pixel, and sprite timing remains explicit instead of collapsing into a scalar line penalty.
6. `4.6` STAT, `LY`, `LYC`, coincidence, and LCD IRQ closure.
   Scope: implement mixed `STAT` readback, live `LY` / `LYC` coincidence, the internal edge-detected LCD STAT line, real VBlank/LCD IRQ timing, and coherence between the exposed mode bits and bus-facing access policy now that variable Mode `3` timing exists.
   Validation: unit tests for mixed readback, immediate `LYC` reevaluation, rising-edge-only LCD STAT requests, and source blocking; integration tests at machine level for `IF` request timing, `STAT.mode`, and bus restriction coherence during mode transitions. Exit criteria: MMIO reads, IRQ requests, and bus policy all observe the same current PPU temporal state, without treating `STAT` sources as unrelated level-triggered checks.
7. `4.7` LCD disable/re-enable, raster restart, and blank-first-frame policy.
   Scope: model `LCDC.7` power transitions, the explicit LCD-disabled state, one documented raster restart state, clean pipeline reset, and the visible blank-first-frame rule after re-enable.
   Validation: unit tests for disabled-state readback, pipeline invalidation, restart-state initialization, and `LY` policy while LCD is off; integration tests for mid-scanline disable/enable, coexistence with DMA-side blocking, and the separation between internal draw restart and panel-visible blank output. Exit criteria: the PPU truly turns off and back on in hardware-facing terms, the implementation does not resume stale FIFOs or stale fetch state, and re-enable does not inherit stale `STAT` edge/coincidence state.
8. `4.8` DMG-family OAM corruption bug.
   Scope: expose the live Mode `2` OAM row, route bus and CPU micro-events into one corruption trigger model, implement the deterministic corruption formulas, include the unusable-area path, and keep DMG-family gating explicit.
   Validation: unit tests for row tracking, first-row immunity, read/write/combined corruption formulas, and DMG-versus-CGB family gating; integration tests with CPU-driven trigger sequences and direct bus accesses during live Mode `2`. Exit criteria: OAM corruption depends on the live Mode `2` row plus routed events rather than opcode blacklists or generic OAM-blocking shortcuts.

#### Phase 4 interleave policy with earlier open TODOs

- Phase `3` leaves no open TODOs, so DMA is not a sequencing blocker for entering Phase `4`.
- The Phase `2` CPU diagnostic TODO that previously turned invalid opcode holes into a silent non-retiring loop is now closed through one explicit diagnostic trap, so deeper Phase `4` ROM or trace debugging no longer fails silently when the CPU fetches a non-ISA opcode such as `$D3`.
- The shared Phase `2` CPU subset that Phase `4.8` depends on is now landed ahead of OAM-corruption closure: `[hli]` / `[hld]`, fetch-time `PC` increments, observable `inc rr` / `dec rr`, and the common address-bearing event model reused by stack/control-flow and interrupt-service paths. The remaining boot-facing MMIO transfer shapes stay deferred because they do not block `4.8`.
- The remaining Phase `2` HALT-edge verification and exact same-cycle `TIMA` / `TMA` reload-write arbitration stay deferred. They should not block early Phase `4` bring-up unless a concrete failing test proves a direct dependency.
- If a Phase `4` subphase lands with a deliberately isolated gap, record the remainder in `Open TODOs` immediately instead of carrying it informally into the next graphics task.

#### Subphase exit rule

Every Phase `4` subphase should end with:

- focused unit tests for the local state machine, pipeline step, or register contract that was introduced
- integration tests when the behavior only becomes meaningful across `ppu`, `bus`, `dma`, `interrupts`, or `machine`
- synthetic VRAM/OAM fixtures or retained trace/snapshot coverage when visible pixel order or timing changes
- `cargo test -q` passing locally at minimum, plus pre-commit checks and `make coverage` whenever the subphase changes shared validation/tooling or other workflow-critical infrastructure
- at least one explicit note about remaining risk when external ROM or oracle validation is still intentionally deferred
- a roadmap TODO recorded immediately if the subphase ships with a concrete uncovered gap

#### Sprite sequencing inside Phase 4

1. Implement Mode 2 sprite selection.
   Scope: vertical match by `Y`, live `LCDC.2` size, OAM order, and the hard `10`-sprite-per-line limit.
   Acceptance criteria: horizontally off-screen sprites still count if their `Y` matches, `8x8` and `8x16` selection are both correct, and OAM discovery order is preserved.
2. Implement DMG OBJ/OBJ priority resolution.
   Scope: choose the winning visible OBJ pixel among overlapping sprite candidates before any BG mixing.
   Acceptance criteria: smaller `X` wins, equal `X` resolves by earlier OAM entry, and BG-over-OBJ does not participate in OBJ/OBJ priority.
3. Implement the object FIFO and transparency rules.
   Scope: object-pixel representation, transparent filler pixels, and OBJ color `0` semantics.
   Acceptance criteria: OBJ color `0` is transparent, transparent OBJ pixels do not block BG, and object FIFO fill behavior is explicit rather than implicit.
4. Integrate object fetch and sprite stalls into Mode 3.
   Scope: in-flight object-fetch state, BG-fetcher interaction, and real sprite-driven Mode 3 lengthening.
   Acceptance criteria: Mode 3 length can increase because of sprite work, BG fetch and object fetch interact on the dot timeline, and the `SCX & 7` plus `X = 0` special path exists as explicit timing-sensitive logic.
5. Implement per-pixel BG/OBJ mixing.
   Scope: popped BG and OBJ pixels, live `LCDC.0` and `LCDC.1`, and DMG BG-over-OBJ semantics.
   Acceptance criteria: BG/OBJ priority is resolved per pixel, BG-over-OBJ is applied only after the winning OBJ pixel is chosen, and DMG `LCDC.0 = 0` behavior remains correct.
6. Cover sprite edge cases and mid-frame toggles.
   Scope: `LCDC.1` fetch cancel, `LCDC.2` size changes, partial vertical clipping, and known `8x16` artifact work.
   Acceptance criteria: object-fetch cancel exists, top and bottom clipping cases are covered, and unresolved `8x16` leak/artifact behavior remains isolated as explicit follow-up work instead of hidden undefined behavior.

#### Window sequencing inside Phase 4

1. Implement basic window activation.
   Scope: WY latch at Mode 2 start, WX trigger during pixel output, and `LCDC.5`-controlled start conditions.
   Acceptance criteria: the window starts only when the WY latch, WX trigger, and window enable are all satisfied, and DMG `LCDC.0 = 0` suppresses window start.
2. Implement fetcher/FIFO reset on window start.
   Scope: BG FIFO clear, fetcher restart, window tilemap selection, and window-local coordinate source.
   Acceptance criteria: the window can begin in the middle of a scanline without recomputing the full line, the visible pixel sequence changes accordingly, and the `WX = 0 && (SCX & 7) > 0` one-dot shortening path is explicit.
3. Implement the internal window line counter.
   Scope: dedicated window Y counter, reset during VBlank, and increment only on scanlines where the window truly starts.
   Acceptance criteria: hiding the window mid-frame can prevent the increment, and status-bar style split usage does not break the chosen window row.
4. Implement WX/WY/LCDC window glitches.
   Scope: glitch pixel behavior around `LCDC.5`, post-start `WX` change behavior, and special handling for `WX = 0` and `WX = 166`.
   Acceptance criteria: the documented glitches exist as pipeline behavior, not framebuffer post-processing, and each case has isolated tests.
5. Integrate window with sprite mixing.
   Scope: BG/window pixel-source transition before final OBJ mixing, without unintended OBJ FIFO resets.
   Acceptance criteria: window start changes the BG/window stream against which OBJ pixels compete, OBJ FIFO state is preserved appropriately, final LCD output still resolves per pixel, and focused tests cover window-plus-sprite interaction without hidden scanline compositing shortcuts.

#### STAT / coincidence / LCD IRQ sequencing inside Phase 4

1. Implement live `LY`, `LYC`, and coincidence state.
   Scope: live `LY` progression through `0..=153`, writable `LYC`, and continuous coincidence evaluation.
   Acceptance criteria: `STAT.2` reflects live `LY==LYC`, coincidence is reevaluated immediately after `LYC` writes, and VBlank lines `144..=153` remain part of the same comparison model.
2. Implement mixed `STAT` readback and enable bits.
   Scope: writable bits `6-3`, live coincidence flag, and live mode bits with LCD-off behavior.
   Acceptance criteria: `STAT` readback composes writable enables plus live bits correctly, mode reads as `0` when LCD is disabled, and software writes cannot overwrite read-only fields.
3. Implement the internal LCD STAT line and rising-edge behavior.
   Scope: OR-composed enabled sources for Mode `0`, Mode `1`, Mode `2`, and coincidence, plus previous-line tracking for edge detection.
   Acceptance criteria: LCD STAT requests occur only on `0 -> 1` transitions of the internal line, Mode `3` is not a direct source, and STAT blocking is reproduced for overlapping enabled conditions.
4. Integrate STAT with the real PPU scheduler and bus-facing access policy.
   Scope: real mode transitions, `LY` progression, Mode `1` entry, and synchronization with VRAM/OAM blocking.
   Acceptance criteria: LCD STAT timing follows the real mode-transition dot, entering VBlank can request both VBlank and LCD STAT Mode `1`, and the mode exposed through `STAT` matches the mode used by the bus for access restrictions.
5. Implement the DMG-family `STAT` write quirk.
   Scope: spurious LCD STAT interrupt behavior on `STAT` writes during Mode `0`, Mode `1`, Mode `2`, and coincidence-active situations.
   Acceptance criteria: the quirk is model-gated to DMG-family behavior, does not appear as a blanket "all `STAT` writes request IRQ" rule, and Mode `3` remains a negative case.
6. Integrate the `STAT`-facing subset of LCD off/on behavior.
   Scope: `LCDC.7` disable/enable effects on reported mode, LCD STAT sources, and VRAM/OAM accessibility, without replacing the broader LCD reactivation work item.
   Acceptance criteria: `STAT.mode = 0` while LCD is disabled, ordinary mode-source behavior is suspended while LCD is off, and re-enable remains compatible with the separate first-full-frame-blank rule handled by the later LCD reactivation block.

#### LCD on/off and reactivation sequencing inside Phase 4

1. Implement real LCD disable.
   Scope: `LCDC.7: 1 -> 0`, explicit LCD/PPU-disabled state, visible LCD-off blank output, and release of ordinary VRAM/OAM mode restrictions.
   Acceptance criteria: disabling LCD stops the active PPU mode scheduler, `STAT.mode = 0`, visible output becomes LCD-off white, ordinary VRAM/OAM mode restrictions are released again without erasing independent DMA-side blocking, and mode-driven LCD STAT requests stop.
2. Implement real LCD re-enable and raster restart.
   Scope: `LCDC.7: 0 -> 1`, one explicit raster-start state, and immediate internal PPU restart.
   Acceptance criteria: the PPU resumes on the shared timeline without an invented startup delay, the re-enable entry point has one documented line/dot/mode source of truth, and the implementation does not resume an old half-finished scanline.
3. Implement visible blank-first-frame behavior.
   Scope: separation between internal pixel generation and panel-visible output after LCD re-enable.
   Acceptance criteria: the internal PPU can start drawing immediately after re-enable while the visible LCD output remains blank for the first full frame, and normal visible output resumes only after that blank frame completes.
4. Reset the pixel pipeline cleanly across LCD power transitions.
   Scope: BG FIFO, OBJ FIFO, fetchers, object-fetch state, window state, and in-progress pixel-mixing state.
   Acceptance criteria: LCD disable invalidates in-flight pixel state explicitly, and LCD re-enable starts from a clean reproducible pipeline instead of resuming stale FIFOs or stale fetch state.
5. Integrate LCD power transitions with bus, LY policy, and mid-scanline writes.
   Scope: synchronized access policy, explicit LY-disabled/re-enable behavior, and immediate `LCDC.7` side effects even outside VBlank.
   Acceptance criteria: bus access policy, `STAT`, `LY`, and the PPU scheduler tell one coherent story during LCD off/on, re-enable does not inherit stale LCD STAT edge/coincidence state, mid-scanline writes remain immediate, and any optional out-of-VBlank warning stays observational only.

#### OAM corruption bug sequencing inside Phase 4

1. Expose the current Mode `2` OAM row.
   Scope: deterministic row tracking for the `20` Mode `2` rows, with one row per `4` dots (`1` M-cycle as a descriptive grouping only).
   Acceptance criteria: the current row is available as live state, matches the real Mode `2` scheduler, and can be consumed by other subsystems without re-deriving it ad hoc.
2. Detect trigger events from bus access and IDU activity.
   Scope: OAM and `FEA0-FEFF` accesses during Mode `2`, plus `16`-bit `inc/dec` activity in `FE00-FEFF`.
   Acceptance criteria: ordinary OAM accesses, `[hli]` / `[hld]`, stack/control-flow sequences, interrupt service, and `PC` increments in OAM all reach one common event model instead of an opcode blacklist.
3. Implement the basic deterministic corruption patterns.
   Scope: distinct read-corruption and write-corruption formulas over the current row and previous row.
   Acceptance criteria: row `0` stays intact, read and write paths remain separate, and the word-copy behavior matches the documented deterministic pattern.
4. Implement combined-event patterns.
   Scope: `write + inc/dec` and `read + inc/dec`.
   Acceptance criteria: `write + inc/dec` collapses to one effective write-corruption result, and `read + inc/dec` uses its dedicated row-restricted path that first mutates the previous row, copies it into the current row and the row two rows before, and then applies the ordinary read-corruption step.
5. Integrate the unusable-area path and model gating.
   Scope: `FEA0-FEFF` reads during Mode `2`, DMG-family enablement, and CGB-family exclusion.
   Acceptance criteria: DMG-family `FEA0-FEFF` reads during the Mode `2` OAM-scan blocked window feed the same controller, outside that window the range keeps its normal DMG readback behavior, and CGB-family models remain unaffected.
6. Close fine validation.
   Scope: row-by-row behavior, first-row immunity, instruction-family coverage, and model-family coverage.
   Acceptance criteria: tests exist for ordinary access triggers, IDU triggers, row exceptions, first-row immunity, and DMG-family positive versus CGB-family negative behavior.

#### Done criteria

- the PPU advances dot-by-dot
- Mode 3 is based on a pixel FIFO rather than deferred scanline rendering
- sprites and window participate inside the real pipeline
- Mode 2 sprite selection, object fetch, OBJ FIFO state, and BG/OBJ mixing are all represented as explicit pipeline behavior rather than scanline-level composition
- DMG sprite priority rules are respected separately for selection, OBJ/OBJ overlap, and BG/OBJ mixing
- window activation, BG-to-window fetch transition, and the internal window line counter are represented as explicit pipeline state rather than as global coordinate remapping
- LCD STAT interrupt generation is represented as one internal edge-detected line driven by live mode/coincidence state rather than as independent level-triggered source checks
- LCD power transitions enter and leave an explicit PPU-disabled state, restart from a defined raster state, and keep the first post-enable blank frame as visible-output behavior rather than a scheduler stall
- DMG-family OAM corruption is modeled from the live Mode `2` row plus bus/CPU micro-events rather than from opcode tables or generic OAM blocking
- STAT, LY, LYC, and LCD IRQs reflect the PPU's real temporal state
- direct-boot LCD-visible state is backed by a coherent internal PPU phase rather than an invented reset-mode shortcut
- bugs and quirks are added on top of an already stable base

#### Risks if oversimplified

- rendering that looks correct but is temporally false
- inability to support real glitches and edge cases
- the need to completely rebuild Mode 3
- incompatibility with tests that depend on FIFO, window, or LCD timings
