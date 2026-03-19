# CPU

## Scope

Own the SM83 CPU execution model: registers, instruction flow, interrupt acceptance, `HALT`, `STOP`, `EI`, `DI`, and CPU-visible timing.

## Hardware model

Model opcode fetch, decode, and execute as explicit phases. Keep instruction semantics separate from timing/accounting decisions so timing refinements do not require rewriting instruction meaning.

For this project, the CPU timing model should be expressed in T-cycles as the fundamental unit. M-cycles may still be useful as a descriptive grouping, but not as the core execution granularity.
Interrupt acceptance, `EI` delay, `DI`, `HALT`, `HALT` bug, `RETI`, and `STOP` should be treated as explicit CPU control-flow states, not as ad hoc patches attached to unrelated bus or interrupt code.
The source of truth should not be "execute opcode, mutate registers, then report an aggregate duration". The source of truth should be an in-flight instruction model made of ordered fetch, read, write, stack, branch, and internal steps that stay synchronized with the shared system timeline.

## Responsibilities

- register file and flag behavior
- instruction decode and execution semantics
- in-flight instruction phase and micro-operation state
- publication of address-bearing read/write and `16`-bit `inc/dec` micro-events when other hardware depends on them
- IME state and delayed enable behavior
- interrupt acceptance and dispatch timing
- operand fetch, memory access, stack access, and branch sequencing
- `HALT`, `HALT` bug, `STOP`, and `RETI` edge cases

## Registers / MMIO

- `AF`, `BC`, `DE`, `HL`, `SP`, `PC`
- `IME`, delayed-IME-enable state, and CPU halt/stop internal state

## Interrupt acceptance baseline

- A pending interrupt condition should be derived from `IE & IF`, not from device-specific flags scattered around the CPU.
- The fixed interrupt priority order is `VBlank > LCD STAT > Timer > Serial > Joypad`.
- The corresponding vectors are `0x40`, `0x48`, `0x50`, `0x58`, and `0x60`.
- The CPU should only accept maskable interrupts at defined points in the instruction-flow pipeline, effectively at instruction boundaries or an equivalent explicitly modeled acceptance point.
- When an interrupt is accepted, the CPU should clear `IME`, clear the selected bit in `IF`, push `PC`, and jump to the matching vector as part of one explicit service sequence.
- The CPU should make that accept-or-not decision only after current-cycle MMIO side effects and interrupt aggregation are already visible; interrupt producers do not bypass that CPU-owned decision point.
- Once accepted, interrupt servicing must consume the documented DMG `20` T-cycles (`5` M-cycles) through the same ordered CPU execution model used for normal stack and control-flow work.
- In the current Phase `2.5` baseline for this repo, interrupt acceptance happens from the explicit instruction-boundary fetch state after scheduler phase `8` has already aggregated requests into `IF`, and the service sequence reuses the same bytewise stack model as `CALL`/`RET`.

## IME, HALT, and STOP baseline

- `IME` is a CPU-internal acceptance gate, distinct from the `IE` register mask.
- `DI` clears `IME` immediately.
- `EI` must not enable `IME` immediately; it should arm a delayed enable that becomes visible only after the following instruction completes.
- `RETI` should restore `PC` through the ordinary return sequence and re-enable `IME` immediately at completion rather than through the delayed-`EI` path.
- `HALT` should be represented as an explicit CPU state distinct from ordinary instruction execution.
- `STOP` should be represented distinctly from `HALT`; even before full DMG/CGB STOP behavior is implemented, the architecture must leave it as a separate CPU control state.
- The architecture must allow `STOP` to be released by an explicit hardware-originated wake path owned by the relevant subsystem, with joypad as the current DMG-family baseline owner, rather than by a frontend-only shortcut.
- The CPU must consume that documented subsystem-owned `STOP` wake policy; it must not define a second local wake rule in parallel.
- For the current repo baseline, the CPU should treat `STOP` wake as the joypad-defined selection-independent button-press wake event, while keeping that wake distinct from any later joypad interrupt service.
- The `HALT` bug must be represented explicitly as a pending effect on the next opcode fetch rather than flattened into a generic "PC did not increment" shortcut.
- In the current Phase `2.6` baseline for this repo, `HALT` entry is resolved during scheduler phase `9`, `HALT` wake and later interrupt service remain separate ordered decisions, and `STOP` resumes only through a joypad-owned wake event instead of a CPU-local or frontend-local shortcut.

## Execution-model baseline

- The CPU should keep an explicit execution state for the current instruction rather than treating each opcode as one opaque step with a total duration attached.
- Each instruction should be decomposable into ordered elementary actions such as opcode fetch, immediate fetch, memory read, memory write, stack byte transfer, branch/control-flow step, and internal time-only step.
- The execution model may be stored either as T-cycle-level subphases or as micro-operations that internally group one M-cycle worth of work while still expanding into `4` explicit T-cycles.
- If such a micro-op model is used, it is only an internal decomposition aid; the architectural timing model must still preserve shared T-cycle observability for CPU, PPU, timer, DMA, interrupts, and bus arbitration.
- Aggregate opcode timing tables may still exist as derived metadata or validation aids, but they must not become a second source of truth that can drift away from the real execution engine.

## Bus-visible instruction flow baseline

- Opcode fetch must be a real bus read at `PC`, not an abstract decode event detached from address routing.
- `PC` should advance according to the real fetch and operand-consumption flow, not by a single hidden increment at the end of the instruction.
- Immediate operands must come from ordered bus reads, with `imm16` low and high bytes fetched separately and in the correct order.
- Indirect accesses such as `(HL)` and `(a16)` must use the ordinary bus path rather than a fast memory shortcut.
- Read-modify-write instructions over memory should be represented as distinct read, transform, and write phases instead of borrowing the register form and patching the duration afterward.
- Conditional instructions must represent taken and untaken paths as different temporal sequences when the documented timing differs.
- Stack operations must be byte-oriented on the bus: `PUSH`, `POP`, `CALL`, `RET`, `RST`, interrupt service, and `RETI` should all reuse the same stack-transfer model.
- Some instructions consume time without an external bus access; the execution model must represent those internal steps explicitly instead of assuming time only passes during reads and writes.
- CB-prefixed instructions should model prefix fetch and extended-opcode fetch as separate ordered steps, with `(HL)` variants layering real memory access and optional writeback on top.
- Flag calculation should be centralized by instruction family and committed when the logical operation resolves, not leaked across unrelated helper steps or applied too early.
- `16`-bit increment/decrement activity on `BC`, `DE`, `HL`, `SP`, and `PC` must remain explicit enough that IDU-driven side effects can be observed even when the instruction does not look like a normal memory access.
- Implicit updates in `[hli]`, `[hld]`, stack/control-flow sequences, interrupt service, and instruction fetch must therefore not be flattened away when their address output matters to the rest of the hardware.

## Timing / accuracy requirements

- Use T-cycle stepping as the baseline execution granularity for this core.
- Treat M-cycles as a derived grouping of four T-cycles, not as the primary scheduling unit.
- Do not hide interrupt and halt behavior behind coarse instruction batching.
- Preserve the ordering between fetch, interrupt checks, and state transitions.
- Keep CPU memory access timing visible at the T-cycle level so VRAM/OAM locking, DMA interaction, and interrupt ordering can be modeled without later restructuring.
- Treat opcode fetch, operand fetch, stack transfer, and indirect memory access as observable time-bearing steps on the shared timeline.
- Do not decode ahead through immediate bytes or CB-prefixed bodies in a way that hides their fetch timing from the rest of the machine.
- Read-modify-write operations over memory must preserve their distinct read and write phases so bus-visible blocking or returned values remain observable.
- Instructions with taken and untaken conditional paths must preserve the documented timing split by using different execution traces rather than a shared path plus a late cycle correction.
- Internal CPU steps with no bus access must still consume real time so external subsystems can advance and interact with the instruction in flight.
- `EI` delay must be tied to instruction completion, not to an unrelated timer or immediate write-back.
- The sequence `EI ; DI` must not leave a window where an interrupt is accepted between those two instructions.
- Interrupt dispatch must not be modeled as an instantaneous jump detached from the CPU timing flow; the service sequence should consume its real CPU-side steps.
- `HALT` wake-up and interrupt dispatch are related but distinct events; waking from `HALT` with `IME = 0` must not be collapsed into automatic interrupt service.
- `STOP` wake-up, joypad interrupt request, and any later interrupt service must remain separable ordered events on the shared T-cycle timeline rather than one collapsed "input resumes CPU" shortcut.
- The CPU should consume the scheduler's already-arbitrated bus result for each micro-op; it must not decide on its own that a blocked VRAM/OAM/cart/MMIO access should succeed anyway.
- CPU wake from `HALT` / `STOP` and CPU interrupt acceptance should happen after the current cycle's device updates are visible, even though the resulting service sequence unfolds across later CPU micro-operations.
- The `HALT` bug condition is `HALT` executed with `IME = 0` and `IE & IF != 0`; it must alter the next fetch without pretending an interrupt was serviced.
- The CPU must expose enough ordered micro-event detail for the DMG-family OAM corruption bug to observe `read`, `write`, `read + inc/dec`, and `write + inc/dec` cases on the shared timeline.
- `PC` increments through the OAM range must remain observable as address-bearing events rather than as a hidden decode-side counter update.

## Dependencies

- bus access API
- interrupt controller state
- T-cycle scheduler or clock source
- model/revision configuration

## Primary references

- Pan Docs
- AntonioND cycle-accurate docs
- Gekkio CPU/material where applicable

## Open-source emulator references

Priority order:

1. SameBoy
2. binjgb
3. GameRoy
4. Danger Boy
5. Gambatte

## Tests

- blargg CPU instruction tests
- Mooneye CPU and interrupt edge-case tests
- tests for opcode fetch under boot-ROM mapping and normal cartridge mapping
- tests for immediate `imm8` and `imm16` fetch ordering and `PC` progression
- tests that distinguish register-only instruction timing from `(HL)` and other memory-indirect variants
- tests for conditional instructions with separate taken and untaken timing, especially `JR`, `JP`, `CALL`, and `RET`
- tests for stack byte order and `SP` update ordering across `PUSH`, `POP`, `CALL`, `RET`, `RST`, and interrupt service
- tests for CB-prefix double-fetch behavior and timing differences between CB register operations and CB `(HL)` operations
- tests for instructions with internal-only steps while timer, PPU, or DMA activity continues externally
- focused tests for `HALT`, `HALT` bug, `STOP`, `EI`, `DI`, `RETI`, and interrupt timing
- interrupt-priority tests with multiple simultaneous pending requests
- tests for correct push of `PC`, clearing of `IF`, and `IME -> 0` on interrupt service
- tests for `EI ; NOP`, `EI ; DI`, `DI ; EI ; NOP`, and pending-IRQ visibility around delayed `EI`
- tests for `HALT` wake-up with `IME = 1`, `IME = 0`, and `IME = 0` plus already-pending interrupt
- tests that interrupt acceptance starts a real `20` T-cycle (`5` M-cycle) service sequence instead of an immediate vector jump
- tests for `STOP` wake-up driven through the relevant hardware source path rather than by directly poking CPU state
- tests for `RETI` re-enabling interrupts and allowing later pending requests to be serviced
- tests that `inc rr` / `dec rr` with `BC`, `DE`, or `HL` in `FE00-FEFF` expose the IDU event needed by the OAM-corruption path
- tests that `[hli]` / `[hld]`, `push` / `pop`, `call` / `ret` / `rst`, interrupt service, and opcode fetch from OAM expose the same micro-event model instead of requiring opcode-specific hacks

## Implementation notes for this repo

- Prefer APIs that expose hardware phases explicitly.
- Keep instruction semantics and timing data separable.
- If helper APIs summarize instruction timing, they should still expand into per-T-cycle execution internally.
- Separate CPU state, decode tables, execution/micro-op planning, ALU helpers, interrupt control flow, and the fine-grained tick engine instead of letting one opcode table own everything.
- Keep explicit state for the instruction in flight, including the current fetch/execute/service phase and any temporary bytes or addresses needed by the next micro-step.
- Keep the configured direct-boot startup snapshot separate from the live CPU
  register file so tests, debugger snapshots, and real execution can compare
  the handoff state against the current in-flight machine state explicitly.
- Prefer decode-time completion for instructions that truly end on the opcode
  fetch machine cycle, and reserve explicit execute steps for instructions that
  need extra immediate fetches, indirect memory traffic, or distinct read and
  write phases, so register-only and `(HL)` forms cannot accidentally collapse
  onto one fake timing path.
- A shape like `FetchOpcode`, `ExecuteMicroOp`, `ServiceInterrupt`, `Halted`, and `Stopped` is a good conceptual fit even if final enum names differ.
- Expose either a `tick_tcycle()`-style API or a micro-step API that expands explicitly into visible T-cycle progress; the scheduler should never need to wait for a whole instruction to retire before other hardware advances.
- Decode and execution should stay distinct enough that base opcodes and CB-prefixed opcodes can reuse the same execution machinery without collapsing their separate fetches.
- Prefer reusable helpers for ALU8, `INC`/`DEC`, `ADD16`, rotate/shift, and bit-operation flag logic instead of scattering flag math across individual opcodes.
- `PUSH`, `POP`, `CALL`, `RET`, `RST`, interrupt service, and `RETI` should share one bytewise stack-transfer model rather than parallel implementations.
- Every CPU-visible memory access, including opcode fetch, immediate fetch, stack traffic, and `(HL)` access, should go through the central bus contract.
- The CPU should own `IME`, delayed-IME-enable state, `halted`, `stopped`, and any `halt_bug_pending`-style fetch modifier state.
- The interrupt controller should own `IE` and `IF` as observable interrupt state, while bus/MMIO wiring exposes those registers at their mapped addresses.
- The CPU should own `stopped` state and the resume point after `STOP`, but the detection of input-driven wake conditions should remain in the relevant hardware subsystem such as joypad.
- A clear split such as `request_interrupt(kind)`, `pending_interrupts()`, and `consume_interrupt(kind)` is preferred over implicit cross-module mutation.
- `RETI` should be implemented as a real instruction with return plus interrupt re-enable semantics, not as `RET` plus an informal external patch.
- Prefer micro-op metadata or callbacks that let the bus/PPU observe "read", "write", and address-bearing `inc/dec` events without hard-coding an opcode blacklist for the OAM corruption bug.
- Keep implicit `HL`, `SP`, and `PC` updates explicit enough that the IDU path can be observed as part of the same T-cycle-accurate CPU model.
- In the current Phase `2.8` baseline for this repo, the scheduler-aligned CPU
  trace should expose `PC`, execution state, `IME`, delayed-`IME` state, and
  the last phase-`5` bus activity for the current T-cycle, distinguishing at
  least opcode fetches from operand and data accesses. Phase `9` should also
  emit a post-wake/post-accept CPU trace so interrupt acceptance is visible on
  the same timeline as the already-visible `IF` state.
- In the current diagnostic baseline for this repo, an unsupported decoded
  opcode enters one explicit `DiagnosticTrap::UnsupportedOpcode { opcode,
  address }` state immediately after the real opcode-fetch bus read retires.
  That trap leaves `PC` at the post-fetch position, keeps the fetched opcode
  visible, and avoids the previous silent non-retiring execute-loop placeholder.
- In the current pre-`4.8` interleave baseline for this repo, the CPU also
  exposes one address-bearing event for the current T-cycle when relevant:
  opcode and operand fetch publish `read + inc` with the post-fetch `PC`,
  `[hli]` / `[hld]` publish `read/write + inc/dec`, `inc rr` / `dec rr`
  publish pure address-bearing `inc/dec`, and stack/control-flow plus
  interrupt-service paths reuse that same combined access-plus-IDU event model
  instead of leaving those updates implicit.

## Real-boot prerequisite matrix

Before the first `RealBoot` execution attempt in Phase `2.4`, treat the DMG
boot path as blocked until this minimum opcode matrix is satisfied or an
explicit narrower boot target is documented.

| Status | Group | Minimum expectation |
| --- | --- | --- |
| Landed by Phase `2.3` | fetch/decode foundation | opcode fetch, `imm8`, `imm16`, `(HL)`, `(a16)`, explicit register-vs-memory timing |
| Landed by Phase `2.3` | control flow | `JR`, `JP`, `CALL`, `RET`, and `RST`, including conditional taken-vs-untaken timing splits |
| Landed by Phase `2.3` | stack traffic | bytewise `PUSH` and `POP`, plus reuse of the same push/pop ordering in `CALL`, `RET`, and `RST` |
| Landed by Phase `2.3` | CB-prefixed control path | explicit second fetch for `0xCB`, plus register and `(HL)` timing distinction for representative prefixed operations such as `RL` and `BIT` |
| Pending before Phase `2.4` | boot-facing MMIO loads/stores | `LDH (a8),A`, `LDH A,(a8)`, `LD (C),A`, `LD A,(C)`, and other MMIO-visible load/store forms used by the boot ROM |
| Landed during Phase `4` interleave | implicit-address transfer forms | `[hli]` / `[hld]` style transfers and the shared address-event publication that Phase `4.8` also consumes |
| Pending before full DMG boot ROM | subtract/accumulator rotates | the remaining boot-visible subtract and non-CB accumulator-rotate families where the production DMG boot ROM depends on them |

Keep this matrix explicit in roadmap and change reports. Real boot should not
quietly start "just to see what happens" while the remaining pending rows stay
unresolved.

Phase `2.4` in this repo currently closes against an explicit narrower boot
target: a synthetic DMG boot ROM that performs representative header reads
through `(a16)`, validates them with `CP d8`, remains in boot on failed
conditional `JR`, and reaches cartridge execution only through an executed
`LD (a16),A` write to `FF50`. Full production DMG boot-ROM execution remains
deferred until the pending MMIO and accumulator-rotate groups above land.

## Recommended implementation order

- implement real opcode fetch at `PC`, integrated with the bus and `PC` progression rules
- implement `imm8` and `imm16` operand fetch steps with decode separated from execution
- implement indirect-memory, stack, and read-modify-write micro-steps such as `(HL)`, `(a16)`, `PUSH`, and `POP`
- implement control-flow instructions with distinct taken and untaken paths
- implement CB-prefix fetch plus extended execution on top of the same engine
- integrate interrupt service, `EI`, `DI`, `HALT`, `HALT` bug, and `RETI` into the same fetch/execute engine
- review internal no-bus steps and remove reliance on detached aggregate opcode durations

## Known pitfalls

- implementing the CPU as "opcode -> handler -> add N cycles" and treating that as the source of truth
- `HALT` bug behavior
- delayed `EI`
- implementing `DI` as delayed when it should be immediate
- treating `IME` as equivalent to `IE`
- interrupt acceptance ordering
- ignoring fixed interrupt priority when several requests are pending
- modeling `HALT` as "sleep until vector jump" instead of separating sleep, wake-up, and service
- decoding immediate bytes or CB-prefixed bodies ahead of their real fetch timing
- collapsing stack traffic into abstract `16`-bit reads or writes
- patching memory-indirect opcodes with extra cycle counts instead of modeling their real bus accesses
- assuming instruction-level stepping is always sufficient
- treating M-cycle totals as enough to model timing-sensitive hardware interaction
- driving the OAM corruption bug from an opcode blacklist instead of from read/write/IDU micro-events
- hiding implicit `HL`, `SP`, or `PC` increments inside helpers so the rest of the machine cannot observe them

## Open questions

- which in-flight execution representation is clearest for this repo: direct T-cycle subphases or micro-ops that group one M-cycle worth of work while still expanding across T-cycles
