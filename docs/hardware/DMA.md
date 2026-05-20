# DMA

## Scope

Own OAM DMA behavior now and leave architectural room for CGB HDMA and related access blocking rules later.

## Hardware model

DMA is not an instant copy when accuracy matters. Represent transfer progress and blocking behavior explicitly.

Treat DMA as another bus actor competing for access over time, not as a side effect detached from normal bus ownership rules.

## Responsibilities

- common DMA-controller contract and active-transfer lifecycle
- `FF46`-triggered OAM DMA start and source-page latching
- OAM DMA transfer state
- DMA kind selection and future per-kind timing policy
- source-page latch and per-step transfer progress
- DMA-side state and policy inputs for bus blocking and visibility rules during transfer
- future HDMA integration points

## Common transfer-architecture baseline

The DMA subsystem should already expose one T-cycle-based transfer model even while the current functional target is only DMG OAM DMA.

The abstraction must keep these dimensions explicit:

- source
- destination
- total remaining work
- operational block size or unit size
- timing policy
- CPU impact policy
- memory-region impact
- advance condition
- lifecycle state

Do not flatten DMA into a generic `memcpy_async(src, dst, len)` helper. OAM DMA, future CGB GDMA, and future CGB HDMA differ in bus ownership, CPU visibility, transfer granularity, and stop/resume rules even when all of them ultimately copy bytes.

## Transfer-family baseline

- Support two conceptual DMA families from the start:
  - full-burst transfers with one explicit in-flight duration on the shared T-cycle timeline
  - windowed or block transfers that only advance in eligible windows such as HBlank
- The shared API must distinguish a fixed total-duration transfer from one that is composed of repeated periodic blocks.
- Current DMG OAM DMA is a full-burst transfer.
- Future CGB GDMA should fit the same full-burst family without redesigning the scheduler.
- Future CGB HDMA should fit the windowed or block-transfer family without redesigning the scheduler.

## Transfer-state and lifecycle baseline

- A shape such as `DmaController` owning one or more typed `ActiveTransfer` values is the intended design direction.
- Each active transfer should expose at least:
  - `kind`
  - `source`
  - `destination`
  - `remaining_bytes`
  - `block_size`, such as future `0x10`-byte HDMA-style blocks
  - `timing_policy`
  - `cpu_impact_policy`
  - `memory_region_impact`
  - `advance_condition`
  - `is_active`
- Lifecycle should be explicit rather than inferred from `remaining_bytes == 0`.
- The common lifecycle should be able to represent `Idle`, `Starting`, `Active`, `Completed`, and future `Cancelled`.
- The subsystem should expose "this transfer affects arbitration or CPU visibility on this T-cycle" separately from "this transfer commits a byte or block on this T-cycle."

## CPU impact policy baseline

- The common contract should already support at least these CPU-impact policies:
  - `NoCpuStallButBusRestriction`
  - `CpuFullyStalledUntilDone`
  - `CpuStalledPerBlock`
- Current DMG OAM DMA maps to `NoCpuStallButBusRestriction`: the CPU keeps executing on the shared T-cycle timeline while DMA publishes the current bus-family conflict that arbitration should apply.
- Future CGB GDMA should be modelable as `CpuFullyStalledUntilDone`.
- Future CGB HDMA should be modelable as `CpuStalledPerBlock`.
- The CPU must not hard-code these policies locally; it should observe the result of central arbitration plus DMA-published state.

## Source and destination contract baseline

- DMA-owned register handling should validate and normalize transfer endpoints inside the DMA subsystem rather than leaving that work to bus call sites.
- OAM DMA should continue to validate the latched source page so the effective source range remains `XX00-XX9F` with `XX` in the documented `00-DF` range.
- The common transfer contract should already leave room for future aligned or forced endpoints, such as CGB VRAM DMA source and destination alignment to `0x10` and a destination forced into VRAM.
- Source and destination restrictions belong to the transfer kind and its controller state, not to frontend register wrappers.

## Registers / MMIO

- `FF46` / `DMA`
- `FF51-FF55` / `HDMA1-5` CGB VRAM DMA register surface

## CGB VRAM DMA register baseline

- `HDMA1` and `HDMA2` latch the VRAM-DMA source address, with `HDMA2` bits `0-3` ignored so the effective source is aligned to `$10`.
- `HDMA3` and `HDMA4` latch the VRAM-DMA destination address, with `HDMA3` bits `5-7` ignored, `HDMA4` bits `0-3` ignored, and the effective destination forced into `$8000-9FF0`.
- `HDMA1-4` are CPU write-only; CPU reads return the unavailable/open value through the shared MMIO contract, but the DMA controller retains the latched normalized endpoints internally.
- `HDMA5` bit `7` selects General-Purpose DMA when clear and HBlank DMA when set; bits `0-6` encode block count minus one, so writes request `$10-$800` bytes.
- General-Purpose DMA starts a full-burst VRAM DMA transfer immediately; the CPU is stalled while the burst copies bytes through the shared DMA work path, the destination VRAM bus is published as occupied, and `HDMA5` reads active until completion returns `$FF`.
- HBlank DMA starts a latched block transfer; the controller copies one `$10`-byte block per eligible visible HBlank window on lines `0-143`, treats any observed Mode `0` on those lines as the eligible HBlank window, completes an already-started block even if the PPU leaves HBlank, does not rearm a second block for the same visible line, and treats the LCD-disabled state as a single eligible window that copies one block until a later distinct window appears.
- HBlank DMA is paused while the CPU is in `HALT`; the active `HDMA5` readback remains stable while halted and the next eligible block starts only after CPU execution resumes.
- Writing `HDMA5` with bit `7` clear while HBlank DMA is active cancels the active transfer, preserves `HDMA1-4`, and leaves `HDMA5` reading bit `7` set plus the low seven bits from the cancel write; the promoted SameSuite DMA fixtures lock the `$00` cancel case as `$80` rather than preserving the pre-cancel remaining count.
- Writing `HDMA5` with bit `7` set while HBlank DMA is active is currently an explicit no-restart policy until hardware-backed mid-transfer restart behavior is modeled; this prevents accidental relatching while block progress is active.
- VRAM DMA source ranges are explicitly limited to ROM, SRAM, and WRAM (`$0000-7FFF` and `$A000-DFFF`) for this slice; unsupported ranges such as VRAM/OAM/MMIO/HRAM copy the explicit garbage value `$FF` until hardware-backed edge behavior is refined.
- VRAM DMA destination overflow is clipped at `$9FFF` so requests starting near `$9FF0` stop at the end of VRAM instead of wrapping into a second address domain.
- Active HDMA latches the CPU-visible source and destination addresses inside DMA state, while the shared bus resolves the live source ROM bank, SRAM bank, WRAM bank, and destination `VBK` mapping for each copied byte; internal tests lock between-block bank/`VBK` changes as live-bus behavior, while the promoted SameSuite DMA rows keep those mappings stable and use upstream framebuffer fixtures as the blocking oracle.

## DMG OAM DMA baseline

- Writing `FF46` starts OAM DMA by latching the written high byte as the source page.
- The nominal source range is `XX00-XX9F`, where `XX` is the latched value from `FF46`.
- Effective OAM-DMA source addresses in `E000-FFFF` follow the common WRAM echo path back into `C000-DFFF`, so `FE00-FFFF` source pages behave like `DE00-DFFF` rather than reading live OAM, MMIO, or HRAM; DMG-family behavior is backed by Mooneye `oam_dma/sources-GS`, while the CGB-family use of this same normalized source path is a Slice 5 model decision locked by internal edge-source tests.
- The destination range is always `FE00-FE9F`.
- A first correct implementation should explicitly track at least `active`, `kind`, `source_high`, `source_addr_current`, `dest_addr_current`, `bytes_remaining`, and `elapsed_dots` or an equivalent byte-phase timing state.
- `FF46` must arm and configure the DMA subsystem; it must not perform the `160`-byte copy immediately as a side effect of the register write.

## `FF46` MMIO contract baseline

- Treat `FF46` as a write-triggered DMA control register with immediate side effects, not as a passive byte that another subsystem polls later.
- The authoritative action of a write to `FF46` is "start OAM DMA with this source page" on that access, not "update a memory-mapped variable that may later cause DMA."
- If a write to `FF46` is visible while a DMG OAM DMA burst is already in flight, it should not cancel the current burst immediately; the current transfer keeps running until the restarted burst reaches its own CPU-visible start seam, at which point the new source page takes over.
- Any MMIO-visible `FF46` readback should come from DMA-owned latched state rather than from a generic bus byte.
- The DMG-family power-on `RealBoot` baseline seeds that DMA-owned `FF46` source-page latch to `0xFF`, matching `gbmicrotest/boot/poweron_dma_000.gb`; this startup value must not start an OAM DMA transfer by itself.
- Internal debug state for DMA may exist separately, but it must not replace explicit in-flight transfer state or the MMIO-owned register view.

## Timing / accuracy requirements

- Describe when CPU and PPU access is blocked.
- Do not hide DMA behind a one-shot memory copy if the target model requires visible timing.
- Keep the design ready for HDMA without rewriting the API surface later.
- DMA progress and blocking should be expressible on the same T-cycle timeline as CPU and PPU activity.
- OAM DMA should be modeled as a `160`-byte transfer whose observable effects unfold over time rather than as a single commit.
- For the current DMG-family target, OAM DMA lasts `160` M-cycles = `640` dots at normal speed.
- On CGB-family hardware, `FF46` OAM DMA latches the current CGB speed profile when the transfer starts; the CPU-visible scheduler still models the same `160` CPU M-cycle OAM DMA body and current `648` T-cycle post-write seam/transfer lifecycle, while the LCD domain observes the normal-speed `640`-dot body as `320` LCD dots in double speed because the LCD domain ticks every other CPU-visible scheduler T-cycle.
- Treat those `160` CPU M-cycles as the hard OAM-DMA requirement across DMG and CGB; CGB double speed changes the LCD-domain dot duration and trace profile, not the DMA byte count, source/destination stepping, or HRAM-accessible CPU bus-arbitration contract.
- On the shared T-cycle timeline, DMG OAM DMA should expose the CPU-visible post-`FF46` start seam from Mooneye's `oam_dma_start` and `oam_dma_timing` ROMs rather than pretending the transfer begins directly on the write T-cycle.
- Hardware fact: Pan Docs still documents the burst body itself as `160` M-cycles = `640` dots.
- Current model decision: keep CPU OAM accesses unrestricted for the full next M-cycle after the `FF46` write completes, then publish the DMA-owned OAM block on elapsed T-cycle `5`.
- Current model decision: keep the first committed DMA byte separate from that visible bus-start edge, with the first byte landing on elapsed T-cycle `8`.
- Current model decision: keep the DMA-visible OAM block through elapsed T-cycle `647` and transition to `Completed` on elapsed T-cycle `648`, which matches the current Mooneye-visible start/end windows without collapsing the post-write seam away.
- Interactions between DMA source access, CPU-visible blocking, and OAM visibility should remain explicit and testable.
- Current model decision and inference: once the start seam ends, DMG OAM DMA publishes the source-bus conflict instead of a blanket "HRAM-only" block. For source pages `80-9F`, the DMA occupies the video RAM bus and OAM, so CPU accesses to VRAM and OAM are blocked while echo/WRAM, cartridge, MMIO, and HRAM stay available. For all other source pages, the DMA occupies the external bus and OAM, so CPU accesses outside HRAM and the explicit `FF46` exception observe DMA-blocked semantics. This keeps the current `FF46` visibility rule, which is backed by Pan Docs' `FF46` R/W register contract and the `mooneye/acceptance/oam_dma/reg_read.gb` oracle, while still treating the general OAM-DMA page's "HRAM only" wording as a coarse summary for ordinary CPU traffic.
- Current model decision and inference: on CGB-family silicon, an external-source OAM DMA burst publishes an external-bus-only CPU restriction instead of the DMG-family broad external-source block; cartridge ROM/RAM accesses still observe the current DMA conflict source, OAM remains blocked by the destination transfer, and internal WRAM, HRAM, and MMIO remain accessible. This keeps the silicon-family DMA policy explicit for CGB-native and CGB-compatible software and is locked by `hacktix/bully.gb (GBC)`.
- Current model decision and inference: on CGB-family silicon, a WRAM-source OAM DMA burst publishes a WRAM-bus-only CPU restriction; WRAM and Echo CPU accesses observe the current DMA conflict source, OAM remains blocked by the destination transfer, and cartridge ROM/RAM plus HRAM/MMIO stay available. This is the Slice 5 internal bus-arbitration contract that prevents WRAM-source CGB OAM DMA from falling back to either the DMG-family broad block or the external-source CGB policy.
- Current model decision and inference: for CGB-family modeling, OAM DMA source pages `E0-FF` use the common OAM-DMA source-normalization path before source-bus classification, so `E000-FDFF`, nominal OAM/unusable `FE00-FEFF`, and nominal MMIO/HRAM `FF00-FFFF` resolve to effective WRAM echo sources `C000-DFFF`; active transfers from those pages publish the same CGB WRAM-bus-only CPU restriction and retained conflict-source traces as ordinary WRAM-source OAM DMA instead of reading raw OAM, unusable memory, MMIO, or HRAM.
- Current model decision and inference: CGB OAM DMA in double speed must not become a generic peripheral multiplier. Internal tests lock that active OAM DMA leaves LCD timing on the CGB LCD-domain gate, leaves HDMA blocks at `32` CPU-visible scheduler T-cycles per `$10`-byte block, and leaves the APU frame sequencer on the speed-domain `DIV-APU` edge instead of gating or accelerating those domains.
- Current model decision: during external-bus DMG OAM DMA, CPU reads and writes that lose arbitration on the occupied bus should resolve against the most recently transferred source byte address (`dma_current_src - 1` style after the first copied byte), not against a generic open-bus placeholder. This is required by curated cases such as `hacktix/bully`.
- The public bus-resolution contract should therefore expose both the CPU's nominal requested target and the effective redirected source-byte target for those accesses; the executed bus path must consume that same resolution instead of re-implementing the redirect as a hidden fast path.
- Current model decision: the DMA view published to the rest of the machine should include the current OAM destination address together with the byte being transferred on that same T-cycle, because the PPU's late Mode `3` metadata reads can observe that destination word during the DMA write window. The PPU may then reconstruct the aligned destination word by combining the live OAM sibling byte with that current DMA byte. This is required by curated cases such as `hacktix/strikethrough`.
- Keep the start seam, the CPU-visible bus-impact onset, and the first-byte commit as separate timing edges in the common DMA model, even when the current DMG OAM transfer keeps them close together.
- DMA destination writes into OAM must still flow through the same central access-arbitration model used elsewhere; do not create a magical OAM backdoor.
- On the scheduler timeline, DMA progress should be advanced before current-cycle bus arbitration and CPU access decisions so the bus sees the live DMA state for that same T-cycle.
- DMA owns transfer progress and source/destination stepping; the bus owns the resulting blocked-access policy observed by CPU and other requesters.
- Do not model DMA at instruction granularity or as a deferred "consume N cycles at the end" event.

## Scheduler integration baseline

- DMA transfers must tick from the global scheduler, not from the CPU execution path.
- On the project's shared T-cycle scheduler, DMA belongs to the autonomous-peripheral portion of the cycle rather than to CPU micro-ops.
- The DMA subsystem should publish current-cycle bus constraints before the bus arbitrates the CPU access for that same T-cycle.
- The DMA subsystem should expose at least:
  - whether CPU access is currently impacted
  - which memory region is currently impacted, such as `Oam`, `Vram`, or no special region
  - whether the transfer performs source or destination work on this T-cycle
- The bus should consult DMA-owned state, not `FF46` or future `HDMA1-5` register contents, when deciding live arbitration behavior.
- A split such as `dma_controller.tick(ctx)`, `dma_controller.bus_constraints(ctx)`, and `dma_controller.commit_memory_writes(ctx)` is a good fit for keeping transfer progress, arbitration publication, and writeback explicit on the T-cycle timeline.

## OAM DMA as the first concrete transfer

- OAM DMA should be implemented as one concrete transfer kind inside the common DMA controller, not as a one-off path outside the subsystem contract.
- That transfer instance should fix these DMG-family properties:
  - fixed size of `160` bytes
  - fixed destination range `FE00-FE9F`
  - linear source and destination stepping
  - documented DMG burst body of `640` dots plus the currently modeled post-write CPU-visible seam
  - CPU impact policy `NoCpuStallButBusRestriction`
  - memory-region impact `Oam`
- OAM DMA remains the first closed functional milestone, but the subsystem contract must already leave room for later block-based and HBlank-conditioned transfers.
- Visual consequences of OAM DMA during Mode `2` or `3` remain owned by the PPU or OAM-side logic; DMA owns transfer state, duration, and region-impact publication.

## Extension seams

- No CGB OAM-DMA source-page range is intentionally implicit for Slice 5: cartridge, VRAM, WRAM, and normalized edge pages `E0-FF` are all model-gated by source-bus tests, copy tests, and retained bus traces; future hardware-backed refinements may change the accepted edge-page mapping only together with replacement tests and roadmap updates.
- The common DMA contract should continue to permit future transfer kinds whose advance depends on global state such as CPU `HALT` or `STOP`, but current GDMA, HDMA, and OAM DMA behavior must stay represented as explicit DMA-controller state rather than one-off bus shortcuts.

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
6. Gambatte

## Tests

- Mooneye DMA tests
- DocBoy `gbmicrotest/dma` ROMs in `gbmicrotest-dmg-extra`
- `FF46` trigger and source-page selection tests
- focused OAM-blocking tests
- DMG timing-window tests that keep the documented `160`-M-cycle burst body visible while also locking the current post-`FF46` CPU-visible start/end seam
- CGB OAM DMA normal-speed versus double-speed tests that keep the `160` CPU M-cycle body stable, show the LCD-domain dot duration difference, keep HRAM accessible during the CGB source-bus restriction, preserve restart speed-profile latching, and prove LCD/HDMA/APU domains do not inherit OAM-DMA speed handling
- CGB GDMA/HDMA tests covering blocking SameSuite framebuffer fixtures, cancel readback, live source ROM/SRAM bank and destination `VBK` mapping between HDMA blocks, visible-HBlank line gating, LCD-off one-block gating, and HBlank exit/seam behavior
- source-bus-aware CPU-access tests during active DMG OAM DMA
- transfer-progress and completion-order tests
- tests that DMA-visible blocking for a T-cycle matches the DMA state produced by that same cycle's scheduler step
- lifecycle-visibility tests covering `Idle`, active start, completion, and future-compatible cancellation hooks
- tests that bus-constraint publication is observable separately from actual byte-copy work on a T-cycle
- tests for common DMA-controller state such as `kind`, `remaining_bytes`, and current CPU-impact policy
- warm-up-seam tests covering the one-full-M-cycle post-write window before CPU OAM blocking begins and the final blocked cycle before `Completed`
- unit tests for a simulated `0x10`-byte block or windowed transfer shape that is not yet wired to CGB MMIO

## Implementation notes for this repo

- Model transfer progress explicitly.
- Keep DMG OAM DMA and future CGB HDMA conceptually separated.
- Prefer designs where DMA consumes bus activity over time so CPU-visible restrictions arise naturally from arbitration rather than a one-shot special case.
- Keep bus arbitration centralized: DMA should request transfer work, while the bus should expose the resulting blocked-access semantics.
- Keep the first-byte start-up seam explicit in the DMA-owned progress model rather than burying it in bus code or by inflating the overall burst duration.
- A scheduler shape where `cpu.tick()`, `dma.tick()`, `ppu.tick()`, `timer.tick()`, and `apu.tick()` all advance on the same T-cycle timeline is the intended baseline, even if orchestration details differ internally.
- Keep `FF46` as the MMIO trigger that configures DMA state; do not bury the whole transfer inside a bus write handler.
- Even if the first implementation only supports `DmaKind::Oam`, structure the subsystem so later `Gdma` and `Hdma` kinds fit without redesigning the contract.
- Keep transfer timing, CPU-impact policy, advance condition, and address validation as DMA-owned concepts rather than scattering them through bus or CPU code.
- Let the bus consume a published DMA constraint view instead of branching on `if dma_kind == ...` across several call sites.
- OAM DMA is the next natural timing deep-dive because it sits at the intersection of CPU accesses, bus arbitration, memory visibility, and PPU/OAM rules.

## First DMG OAM DMA milestone

- add a dedicated DMA subsystem with explicit inactive/active transfer state and `FF46` start handling
- integrate `dma.tick()` into the shared T-cycle scheduler
- enforce DMG source-bus-aware CPU access semantics while OAM DMA is active
- route DMA reads and OAM writes through the same central arbitration model used by the rest of the system
- add focused tests for total duration, correct `160`-byte copy, CPU blocking, source-bus-specific accessibility, and LCD-enabled interaction

## Known pitfalls

- implementing DMA as an invisible instant copy
- suspending the CPU completely instead of modeling its restricted bus access during DMA
- forgetting access restrictions during transfer
- routing OAM DMA through a special path that bypasses bus arbitration
- letting `FF46` perform the full transfer immediately instead of only arming DMA state
- hard-coding the subsystem as a one-off DMG `FF46` copy path that cannot grow into CGB DMA variants

## Open questions

- where DMA scheduling should attach to the future scheduler/clock domain
