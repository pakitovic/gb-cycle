# BUS

## Scope

Own address decoding, subsystem routing, and visible memory access ordering.

## Hardware model

The bus is not just a convenience table. It is where address ownership, access restrictions, and observable ordering become explicit.

Even in a DMG-only implementation, treat VRAM, WRAM, OAM, cartridge space, HRAM, and MMIO as distinct controlled regions rather than one rigid flat memory block.
Address alone is not enough: the bus must also consider the current temporal hardware state such as PPU mode, LCD enable state, DMA activity, boot ROM mapping, console model, and later CGB banking or speed mode.

## Responsibilities

- route reads and writes by address range
- expose MMIO ownership clearly
- keep arbitration and blocking rules visible
- apply access rules based on the current hardware state, not only the address
- let the CPU perform generic bus accesses without embedding device-specific lock rules in CPU code
- distinguish requester-specific access semantics when CPU, DMA, or other actors do not obey the same rules
- coordinate dynamic mapping between boot ROM, cartridge ROM, and later model-specific extensions
- consume subsystem-owned state such as PPU mode, DMA progress, and boot-ROM enable state when deciding the observable result of an access
- consume DMA-published constraint state from the common transfer controller rather than re-deriving live DMA policy from MMIO register values or ad hoc per-kind branches
- delegate all cartridge-owned accesses through one stable cartridge-device contract instead of embedding per-MBC logic in the bus

## Registers / MMIO

- full memory map routing
- shared access to cartridge, VRAM, WRAM, OAM, HRAM, and MMIO registers
- startup mapping of boot ROM over `0000-00FF` and later cartridge handoff

## DMG region-decode baseline

- The bus should expose one central decode path covering the full `0x0000-0xFFFF` address space.
- That decode path should distinguish at least these regions:
  - `0x0000-0x3FFF`: fixed cartridge ROM, with boot-ROM overlay when active
  - `0x4000-0x7FFF`: switchable cartridge ROM
  - `0x8000-0x9FFF`: VRAM
  - `0xA000-0xBFFF`: cartridge-visible RAM or cartridge-owned external hardware
  - `0xC000-0xCFFF`: WRAM bank 0
  - `0xD000-0xDFFF`: WRAM additional region, linear on DMG and future-bankable on CGB
  - `0xE000-0xFDFF`: echo RAM alias of `0xC000-0xDDFF`
  - `0xFE00-0xFE9F`: OAM
  - `0xFEA0-0xFEFF`: unusable / prohibited region
  - `0xFF00-0xFF7F`: MMIO registers
  - `0xFF80-0xFFFE`: HRAM
  - `0xFFFF`: `IE`
- The bus should resolve accesses from address plus current hardware context, not from address alone.
- CPU, DMA, and other actors must not bypass that central decode path with direct "fast" access to backing arrays.

## Domain-oriented architecture baseline

- Treat the routed target as `domain + region + offset`, not only as one flat region enum.
- The current DMG-first preferred domain split is `BootRom`, `Cartridge`, `Vram`, `Wram`, `Oam`, `IoHram`, and `Unusable`.
- If a docboy-like internal domain is introduced, prefer `IoHram` or `Internal` naming over `CpuBus`; `FFxx`, `HRAM`, and `IE` belong together there, while WRAM should stay explicit for future CGB expansion.
- The address router or MMU should only resolve nominal routing and boot-overlay ownership. It must not decide live PPU Mode `3` blocking, DMA conflict policy, or other timing-dependent outcomes.
- Timing-dependent blocked-access behavior remains the responsibility of arbitration plus the owning domain or device contract.
- PPU and DMA may consume bus-originated OAM and VRAM views instead of raw storage pointers, but those views must still come from the same bus/domain layer and remain synchronized to the shared T-cycle timeline.

## Region contract baseline

### Cartridge fixed ROM `0x0000-0x3FFF`

- This region should be owned by the cartridge device, not by internal console memory.
- While boot ROM is mapped, the relevant low part of this region must be overlaid by boot firmware routing; once boot ROM is unmapped, reads must return cartridge ROM again.
- Reads should return bytes through the cartridge device's ROM-read contract, not through a bus-local ROM array or mapper-specific switch.
- Writes must not attempt to modify ROM contents; they should be delegated to cartridge/MBC control semantics through the cartridge device.
- The cartridge header at `0x0100-0x014F` should become visible through normal cartridge routing once boot ROM no longer covers it.

### Cartridge switchable ROM `0x4000-0x7FFF`

- This region should also be owned by the cartridge device.
- Active-ROM-bank selection belongs to cartridge/MBC logic, not to the generic bus.
- Reads should come from the active cartridge ROM bank as selected by the cartridge implementation.
- Writes should be treated as MBC control writes where the cartridge type requires that behavior, not as ROM writes, and the bus should not inspect `0x0147` itself to decide the mapper rule.

### VRAM `0x8000-0x9FFF`

- VRAM should be treated as a dedicated graphics-memory region rather than generic RAM with no access policy.
- CPU reads and writes to VRAM must respect PPU access timing.
- On DMG, CPU VRAM access should be allowed in Modes `0`, `1`, and `2`, and blocked during Mode `3`.
- With the LCD disabled through `LCDC.7 = 0`, ordinary PPU-mode VRAM restrictions should be lifted immediately because the active raster pipeline is no longer running.
- When blocked, CPU writes should be ignored and CPU reads should return the blocked-access result rather than the stored VRAM byte, typically `0xFF`.
- The PPU and CPU must not have incompatible direct paths to VRAM that bypass shared access policy.

### Cartridge RAM / external range `0xA000-0xBFFF`

- This range should be owned by the cartridge device, not by internal console RAM.
- The bus should delegate both reads and writes here to cartridge/MBC logic.
- Semantics may vary by cartridge type: absent RAM, mapper-local RAM such as MBC2 internal RAM, enable-controlled RAM, banked RAM, RTC, or other external hardware.
- Behavior for unmapped or wrapped RAM banks should be defined by the active cartridge/MBC implementation, not hard-coded in generic bus code.
- The bus should not infer cartridge-RAM presence or shape from `0x0149` on its own; the loaded cartridge device already owns that decision, including mapper-local RAM special cases such as MBC2.

### WRAM `0xC000-0xDFFF`

- WRAM should be treated as internal console RAM.
- On DMG, `0xC000-0xDFFF` should behave as a linear `8 KiB` working RAM region.
- Future CGB bankability of `0xD000-0xDFFF` should remain an extension seam without changing current DMG behavior.
- Initialization policy for WRAM contents is separate from access semantics; once initialized, reads and writes should behave like normal internal RAM.

### Echo RAM `0xE000-0xFDFF`

- Echo RAM must not be modeled as independent storage.
- It should behave as a real alias of `0xC000-0xDDFF`.
- Writes through echo addresses should affect the mirrored WRAM bytes and vice versa because the observable storage is shared.
- The alias relationship should be expressed in bus decode and routing rather than by duplicating memory buffers.

### OAM `0xFE00-0xFE9F`

- OAM should be treated as a dedicated sprite-attribute region, not as always-accessible RAM.
- CPU OAM access must obey both PPU timing and DMA-related bus policy.
- On DMG, CPU OAM access should be blocked during PPU Modes `2` and `3`.
- With the LCD disabled through `LCDC.7 = 0`, ordinary PPU-mode OAM restrictions should be lifted immediately even though other actors such as OAM DMA may still impose their own access rules.
- During blocked periods, CPU writes should be ignored and CPU reads should return the blocked-access result instead of the stored OAM byte.
- On affected DMG-family hardware, CPU-visible OAM reads or writes during Mode `2` should also feed the OAM-corruption trigger path while still observing the ordinary blocked-access result.
- During Mode `3`, blocked OAM access should remain blocked without automatically implying the DMG OAM corruption bug.
- OAM DMA should write into the same underlying OAM storage while still participating in the same central arbitration model.

### Unusable area `0xFEA0-0xFEFF`

- This region must not be modeled as free RAM.
- For the current DMG-family target, it should have explicit revision-aware behavior instead of a placeholder array.
- For the current repo baseline on `DMG0`, `DMG`, and `MGB`, reads should return `0x00` outside OAM-blocked periods and `0xFF` during OAM-blocked periods.
- If later model coverage or hardware evidence requires refinement for a specific revision, keep that change model-gated here rather than falling back to generic RAM semantics.
- On affected DMG-family hardware during the specific Mode `2` OAM-scan block, reads from this range should also enter the same OAM-corruption trigger path used for OAM reads.
- Other causes of temporary OAM unavailability must not be treated as an automatic OAM-corruption trigger for `FEA0-FEFF`; the bug hook belongs to the Mode `2` path.
- The region should stay explicitly connected to later OAM corruption bug work rather than being treated as unrelated filler space.
- Writes here should not behave like ordinary RAM writes.

### MMIO `0xFF00-0xFF7F`

- MMIO must not be modeled as a generic `128`-byte RAM block.
- Each register should have explicit read, write, and side-effect semantics.
- The MMIO table should distinguish at least read-only, write-only, read/write, and mixed or reserved-bit behavior.
- `FF46` should remain the OAM-DMA trigger and `FF50` the boot-ROM mapping control.
- CGB-only registers should stay under an explicit DMG absent/stub readback policy instead of surfacing accidentally as RAM.

## MMIO contract baseline

- Do not model an MMIO register as "stored byte plus optional mask" unless the hardware behavior truly is that simple.
- Treat each MMIO register as an interface owned by one subsystem, with its own read behavior, write behavior, and side effects.
- Distinguish the value visible to MMIO reads from any internal latched fields and from the side effects caused by an access.
- Keep one source of truth for MMIO semantics. Avoid shadow register behavior implemented separately in CPU, bus, timer, PPU, APU, DMA, or frontend code.

## MMIO descriptor baseline

- Every address in `0xFF00-0xFF7F` and `0xFFFF` should resolve to an explicit descriptor or equally explicit dedicated handler.
- That descriptor should make the following properties explicit:
  - owning subsystem
  - access class
  - readable bits
  - writable bits
  - dynamic read-only bits
  - write-only or read-suppressed bits
  - reserved or forced bits
  - read side effects
  - write side effects
  - model-specific availability such as shared, DMG-only, CGB-only, absent in the current model, or stubbed
- The descriptor should support readback composition from latched bits, dynamic bits, forced bits, and unimplemented bits rather than assuming all visible bits come from one stored byte.

## MMIO access-class baseline

- The minimum MMIO access taxonomy should be `ReadOnly`, `WriteOnly`, `ReadWrite`, and `Mixed`.
- `Mixed` registers must keep per-bit or per-field behavior explicit; they should not be downgraded to generic `ReadWrite` storage plus one coarse mask.
- Write-only readback policy must be explicit per register or per field. Do not rely on an accidental project-wide default.
- Writes to read-only bits inside mixed registers should follow register-specific masking or ignore rules rather than mutating storage accidentally.

## MMIO execution-order baseline

- The bus should decode the address and delegate to the owning subsystem; it should not embed the full internal logic of `JOYP`, `STAT`, `DIV`, `NR52`, or other subsystem-owned registers.
- CPU code should perform ordinary bus/MMIO accesses and let the owning device decide what the register read or write means.
- MMIO side effects should occur at the time of the actual access on the shared T-cycle timeline, not in a deferred end-of-instruction cleanup pass.
- Reads of dynamic MMIO registers should sample the subsystem's live hardware state at that exact access point.

## Arbitration layering baseline

- Bus arbitration should be split into two explicit layers:
  - decode and nominal ownership from address plus current mapping state
  - requester-aware access policy over that resolved region or owner
- Boot ROM overlay versus cartridge visibility belongs to decode and ownership resolution, not to a CPU-local shortcut.
- Transfer engines must win over CPU accesses wherever the hardware documents that precedence; on DMG, active OAM DMA is the mandatory case.
- After nominal ownership is known, the owning region or device may still block or modify the access according to live policy, such as VRAM in Mode `3`, OAM in Mode `2/3`, `FEA0-FEFF`, disabled cartridge RAM, or cartridge-selected RTC behavior.
- CPU, DMA, and any future transfer engine must all use this one central arbitration path; no caller-specific fast path may bypass decode or access policy.
- On the shared scheduler timeline, the arbitration decision for a T-cycle should see the already-updated current-cycle DMA and PPU state before the CPU micro-operation issues its access for that same T-cycle.
- The bus should consult DMA-owned published constraints such as CPU impact, region impact, and current-cycle transfer activity instead of peeking at `FF46` or future `HDMA1-5` register state directly.
- DMA policy questions such as "external bus blocked", "video bus blocked", "fully stalled until done", or "stalled only during a block" belong to the DMA subsystem; the bus should only apply the resulting requester-visible constraints.
- For the current DMG OAM-DMA baseline, keep one explicit exception to the ordinary CPU-side DMA block: `FF46` itself must stay readable and writable so active-DMA readback and restart behavior remain visible through the MMIO path.

## Model-aware MMIO baseline

- Register availability must stay model-aware rather than being inferred from whether a backing field exists today.
- In DMG mode, unimplemented CGB-only registers should return the correct DMG fallback read value, typically `0xFF`, through the ordinary MMIO path.
- Writes to unavailable CGB-only registers in DMG mode should follow an explicit ignored-or-stub policy; they must not mutate fake storage just because the address is in `FFxx`.

### HRAM `0xFF80-0xFFFE`

- HRAM should be modeled as a dedicated internal RAM region distinct from WRAM and MMIO.
- On DMG, CPU HRAM access should remain available during OAM DMA regardless of which source bus the transfer currently occupies.
- During an external-bus OAM-DMA conflict, the current baseline keeps CPU VRAM plus `FF46` accessible while other CPU accesses observe DMA-blocked semantics.
- During a video-bus OAM-DMA conflict, the current baseline keeps CPU access to non-VRAM, non-OAM regions available while VRAM and OAM observe DMA-blocked semantics.
- During either DMG OAM-DMA conflict shape, the dedicated `FF46` MMIO path should remain accessible for DMA readback and restart writes.
- HRAM initialization policy is separate from its access semantics.

### `IE` at `0xFFFF`

- `0xFFFF` should decode to the interrupt-enable register, not to ordinary RAM.
- Its read and write behavior should be routed through the interrupt-controller/MMIO path rather than through generic high-memory storage.
- The fact that `IE` lives outside `0xFF00-0xFF7F` should remain explicit in the bus decode.

## Timing / accuracy requirements

- Bus-visible ordering must remain explicit.
- Access restrictions from PPU and DMA must not be hidden.
- Boot-ROM overlay and cartridge handoff must be represented as observable routing behavior, not as a CPU-local switch or a post-boot jump shortcut.
- Cartridge ROM-space writes and cartridge external-range accesses should remain ordinary ordered T-cycle bus transactions whose meaning is delegated to the cartridge device.
- OAM decisions must consider address, LCD enable state, PPU mode, and OAM DMA state together rather than as unrelated checks.
- OAM access blocking during PPU Mode 2 must be represented as observable bus behavior, not as a render-only detail.
- During PPU Mode 3, both OAM and VRAM access restrictions must be represented as observable bus behavior.
- During DMG OAM DMA, CPU accesses should retain normal HRAM behavior while DMA-published source-bus conflicts determine whether the blocked set is "everything except HRAM, VRAM, and `FF46`" or "VRAM and OAM only".
- DMA-visible blocking and DMA data movement should remain separable on the T-cycle timeline; a transfer may affect CPU-visible access policy on a cycle even if no byte commit occurs on that same cycle.
- With LCD disabled, access rules should return to the hardware state expected for LCD-off behavior.
- LCD-off accessibility should remove ordinary PPU mode locks, but it must not erase independent blocking rules coming from DMA or any later bus actor.
- The same PPU-disabled state that makes `STAT.mode` read as `0` should also be the state the bus uses to release ordinary VRAM/OAM mode restrictions.
- Mid-scanline writes to `LCDC.7` should therefore be able to change VRAM/OAM accessibility immediately on the shared timeline rather than at scanline or frame end.
- The bus must distinguish ordinary blocked OAM semantics from the DMG-family OAM corruption bug; not every blocked OAM or unusable-area access should trigger corruption.
- CPU-originated OAM or `FEA0-FEFF` access attempts during Mode `2` on affected models should enter the OAM-corruption event path using the live current row reported by the PPU.
- CPU-provided address-bearing `16`-bit inc/dec events in `FE00-FEFF` should also route into that same corruption controller even when no ordinary memory access occurs.
- In the current Phase `4.8` baseline, classify ordinary CPU access triggers through the same bus-resolution path used for blocked-access semantics, so Mode `2` OAM reads/writes and Mode `2` unusable-range reads reuse the existing blocked-reason model instead of bypassing it with a parallel check.
- In the current Phase `4.8` baseline, keep pure IDU `inc/dec` trigger routing separate from normal access resolution: reconstruct the driven pre-update address from the CPU event, require live LCD-enabled Mode `2`, and do not let Mode `3` OAM blocking imply corruption automatically.
- When an access is blocked, the bus should model the correct observable result for that situation instead of falling through to normal RAM semantics.
- CPU opcode fetch, immediate fetch, stack traffic, and read-modify-write memory operations should appear as ordinary ordered bus accesses, not as post-instruction aggregated effects.
- MMIO reads and writes should remain ordinary ordered bus transactions whose visible result and side effects depend on the exact temporal hardware state at that access point.
- `STAT` mode visibility, VRAM/OAM access policy, DMA blocking, and other bus-facing dynamic state must stay coherent within the same T-cycle; the bus must not read one subsystem snapshot while software observes another.
- `SkipBoot` should begin with the same ordinary routing rules the machine would have after handoff, not with a hidden "skip mode" that bypasses normal boot-ROM and cartridge visibility logic.
- In DMG mode, reads from CGB-only registers that are not functionally implemented should return `0xFF` through the normal MMIO routing path rather than through ad hoc call-site checks.
- Each region should have explicit read, write, blocked-access, and model-specific policy rather than being treated as RAM or ROM with only a different backing store.

## Dependencies

- memory/MMIO map
- boot subsystem state
- model/revision configuration
- cartridge/MBC
- PPU, DMA, timer, interrupt controller, joypad, serial, APU

## Primary references

- Pan Docs memory map sections
- AntonioND timing material
- Gekkio documentation and tests

## Open-source emulator references

Priority order:

1. SameBoy
2. docboy
3. binjgb
4. GameRoy
5. Mooneye GB
6. Gambatte

## Tests

- Mooneye memory and MMIO behavior tests
- subsystem-specific access restriction tests
- tests for blocked reads returning the expected observable value and blocked writes being ignored where applicable
- tests for requester-specific behavior during OAM DMA, including CPU HRAM access, source-bus-specific blocking, and DMA-driven OAM writes
- tests for LCD-off VRAM and OAM accessibility and for immediate access-policy change on `LCDC.7` transitions
- tests that LCD-off accessibility and DMA-specific blocking compose correctly instead of one silently erasing the other
- tests for boot-ROM overlay before `FF50` and cartridge visibility after `FF50`
- tests that the next fetch after boot-ROM unmapping already observes cartridge routing
- direct-boot routing tests that verify the ordinary cartridge ROM map is visible again after startup, including `0x0000`, `0x0100`, and mapper-controlled ROM regions where applicable
- tests that all cartridge-owned regions `0x0000-0x7FFF` and `0xA000-0xBFFF` route through the loaded cartridge device contract without bus-side mapper heuristics
- DMG-mode MMIO tests that verify CGB-only registers read back as `0xFF`
- full-range decode tests that ensure every address in `0x0000-0xFFFF` maps to an explicit owner and policy
- tests that ROM-region writes are delegated to cartridge/MBC control rather than treated as memory writes
- bidirectional alias tests between WRAM and echo RAM
- tests for `0xFEA0-0xFEFF` DMG-family read behavior inside and outside OAM-blocked periods
- tests that Mode `2` OAM accesses and Mode `2` `FEA0-FEFF` reads on affected DMG-family models trigger the OAM-corruption path while preserving the documented blocked-access readback
- tests that blocked OAM access in Mode `3` does not trigger the DMG-family OAM corruption bug automatically
- tests that CPU-provided IDU `inc/dec` events in `FE00-FEFF` reach the same corruption controller without requiring a normal memory read or write
- tests that `0xFFFF` routes to `IE` rather than HRAM or generic MMIO backing
- full MMIO descriptor-coverage tests for `0xFF00-0xFF7F` and `0xFFFF`
- tests for write-only readback policy and mixed-register bit composition through the routed MMIO path
- tests that MMIO side effects such as `DIV` reset, `FF46` DMA start, and `FF50` unmapping occur on the access itself rather than a later deferred phase
- tests that decode / ownership resolution and access-policy resolution stay distinct and observable
- tests that DMA precedence over CPU access is decided through the same central arbitration path rather than a CPU-local special case
- tests that `STAT`-visible mode and the bus's VRAM/OAM restrictions remain coherent on the same T-cycle

## Implementation notes for this repo

- Keep cartridge logic decoupled from the rest of the bus.
- The bus should depend on a base cartridge interface such as `read_rom`, `write_rom`, `read_ram`, `write_ram`, and metadata accessors, not on concrete `Mbc1` or `Mbc3` types.
- Favor explicit maps and handlers over opaque indirection.
- Treat the bus as both an address decoder and an access arbiter.
- Keep one source of truth for address decode plus access policy; do not let per-subsystem shortcuts become shadow decoders.
- A pure address-router plus requester-facing domain views is the preferred long-term structural shape for this repo's bus, as long as the router itself stays timing-agnostic.
- In the current repo, prefer `bus.rs` as a narrow facade plus focused child modules such as `map.rs`, `router.rs`, `iohram.rs`, `wram.rs`, `video.rs`, and `view.rs` instead of one monolithic file that mixes decode, MMIO, storage, and video ownership.
- A bus context or equivalent state bundle is a good fit for carrying model, PPU mode, LCD enable, DMA activity, boot ROM mapping, and later CGB-specific selectors.
- A caller-aware access split or equivalent internal distinction between CPU-initiated and DMA-initiated accesses is recommended when the observable rules differ.
- Let subsystems define the state that causes restrictions or remapping, but keep the final blocked-access or routing decision in bus-facing handlers or in explicit domain-local access helpers reached from that one bus path.
- A DMA-facing query such as `bus_constraints()` plus a separate transfer-commit path is a good fit for keeping arbitration policy separate from byte-copy mechanics.
- `docboy` is an approved structural oracle for this domain split, especially for DMG PPU-facing `VRAM/OAM` views and explicit video-bus acquisition or release timing; use it as a cross-check, not as a code-copy source.
- For the current DMG-first baseline, `IoHram` should own `FFxx`, `HRAM`, and `IE` routing plus MMIO handler dispatch, while `Wram` stays separate and `video.rs` owns `VRAM/OAM` storage plus acquisition state.
- Keep a scheduler-visible ownership sync step or equally explicit equivalent for `VRAM/OAM`; the router must not guess live PPU or DMA ownership on its own.
- Requester-facing OAM/VRAM views may expose real `acquire` / `release` operations, but observable policy must still stay coherent with the shared T-cycle scheduler rather than relying on ad hoc local borrowing conventions.
- Prefer a centralized MMIO descriptor table or equivalent routed register map over scattered `match` blocks that each know only part of a register's semantics.
- Let subsystem-owned handlers compose readback from live state, latched state, and forced bits; do not teach the bus to fake those register internals.
- Do not special-case CPU opcode fetch, operand fetch, or stack accesses outside the common bus contract; they should use the same routed access path as any other CPU-visible memory transaction.
- A routed helper such as `notify_oam_corruption_event(kind, addr)` is a good fit once CPU micro-ops and the PPU's current Mode `2` row are available; let the bus classify address-space triggers but not own the corruption formulas themselves.
- Treat `FF46` as the trigger that configures the DMA subsystem; do not implement OAM DMA by performing a direct `160`-byte copy inside the bus write path.
- Treat `FF50` as the trigger that changes boot-ROM mapping state; do not model real boot completion as a synthetic `PC = 0x0100` event outside the bus and CPU execution flow.
- Design region ownership so future CGB additions can extend VRAM banking, WRAM banking, extra I/O registers, and HDMA without replacing the bus contract.
- Prefer region controllers or explicit handlers over hard-coded assumptions like "DMG only has one VRAM shape forever".
- The bus should model boot ROM mapping as a first-class routing rule, including the later `FF50`-controlled unmap to cartridge ROM.
- The DMG-family next-fetch handoff after `FF50` should already be modeled in a way that can later extend to CGB's split boot-ROM mapping while keeping the cartridge header window visible.
- Cartridge type detection, ROM-size decoding, RAM-size decoding, and header validation belong to cartridge loading rather than to the bus decode path.
- In `SkipBoot`, boot ROM mapping should simply start disabled while leaving `FF50` and the ordinary mapping logic intact.
- After `SkipBoot`, the bus should expose the normal cartridge ROM layout over `0x0000-0x7FFF` rather than a special reduced direct-boot map.
- Keep DMG-versus-CGB MMIO readback policy in the routed register map rather than spreading `if cgb` checks through unrelated subsystems.
- Avoid boot-ROM mapping code that assumes firmware always occupies exactly one small contiguous prefix of the address space.
- Leave room for model-specific boot firmware windows that are not a single contiguous DMG-style range.
- Keep blocked-access behavior inside bus-facing region handlers such as VRAM/OAM access paths rather than teaching the CPU about those rules.
- A region-description shape that makes owner, read behavior, write behavior, blocked semantics, and future extension points explicit is preferred over ad hoc nested matches.

## Known pitfalls

- accidental coupling between unrelated devices
- hiding blocked reads/writes behind generic memory helpers
- treating requester identity as irrelevant when CPU and DMA need different observable access rules
- freezing the MMIO map behind abstractions that are hard to extend for CGB-only registers
- reducing mixed MMIO registers to "byte plus mask" storage and losing per-bit semantics
- treating the bus as a static memory map without temporal arbitration
- adding a special direct-boot routing shortcut instead of initializing the normal post-boot mapping state
- modeling the unusable area `0xFEA0-0xFEFF` as ordinary RAM
- duplicating echo RAM storage instead of routing it as an alias of WRAM
- conflating any blocked OAM access with the Mode `2`-specific DMG OAM corruption bug
- hard-coding OAM corruption in the bus as an opcode list instead of routing micro-events into the controller that owns the formulas

## Open questions

- whether scheduler ownership should live above the bus or beside it
