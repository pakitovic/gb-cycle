# Phase 2 — CPU and real temporal control

10. **Exact CPU core at the fetch / execute / memory access level**
11. **Timer**
12. **Interrupts: IME, IE, IF, EI, DI, priority, acceptance timing**
13. **HALT, STOP, and HALT bug**

#### Goal

Build a truly temporal CPU core, where observable behavior emerges from internal steps compatible with the T-cycle scheduler.

#### Modules involved

- `cpu/`
- `timer/`
- `bus/`
- `scheduler/`
- `debugger/`

#### Deliverables

##### CPU core

- fetch / decode / execute at the T-cycle level
- internal micro-sequences per instruction when needed
- explicitly modeled reads and writes
- correct handling of relevant internal states

##### CPU / boot integration

- real boot-ROM execution through the same CPU core and scheduler used after startup
- real `FF50` write causing cartridge handoff on the next fetch
- logo/checksum outcomes emerging from executed boot-ROM code rather than emulator-side validation
- correct model-visible cartridge-entry state after real boot

##### Timer

- DIV
- TIMA
- TMA
- TAC
- edge timing integrated with the real system clock
- timer interrupt request generation
- direct-boot timer hidden-state synthesis coherent with the visible post-boot timer snapshot

##### Interrupts and CPU states

- IE and IF registers
- IME latch
- real temporal effect of EI and DI
- interrupt priority
- interrupt acceptance timing
- explicit separation between source request, `IF` aggregation, CPU wake, and CPU interrupt acceptance
- HALT
- STOP
- HALT bug

#### Done criteria

- the CPU core does not depend on an oversimplified full-instruction abstraction
- instructions generate their real bus accesses
- the timer advances with the global scheduler
- interrupts and HALT are integrated into the real execution flow
- source requests become visible in `IF` before the CPU accepts them, and timer keeps its delayed request timing instead of being flattened into same-cycle overflow service
- direct-boot timer state does not fake `DIV` or related registers through disconnected visible-only initialization
- real boot executes through the same CPU fetch/decode/execute engine used for the rest of the machine
- real boot reaches cartridge code only through an executed `FF50` write and next-fetch handoff
- invalid boot-logo or header-check cases remain in boot instead of handing off to the cartridge
- tracing can observe fetches, accesses, and IRQ acceptance

#### Recommended sequencing inside Phase 2

Phase 2 should be executed as narrow subphases. No subphase counts as closed unless its local acceptance criteria land together with focused automated coverage and move the phase-level done criteria forward without reintroducing instruction-level shortcuts or hidden timing.

1. `Phase 2.1` - CPU execution plumbing and live register state.
   Acceptance criteria: the CPU stops being a startup-state-only stub, keeps a live register file plus explicit in-flight execution state, performs opcode fetch as a real bus read at `PC`, advances `PC` through explicit fetch flow, and exposes traceable per-T-cycle CPU state such as fetch, execute, service-interrupt, halted, and stopped without yet claiming broad opcode coverage. Validation gate: focused unit tests cover register-file initialization, opcode fetch at `PC`, explicit `PC` progression, deterministic micro-step traces, and scheduler-visible CPU state transitions under `SkipBoot`.
2. `Phase 2.2` - Memory-visible instruction bring-up.
   Acceptance criteria: the first instruction families run through ordered bus accesses rather than aggregate duration tables, `imm8` and `imm16` fetches remain explicit and correctly ordered, register-only and `(HL)` forms no longer share one flattened timing path, and memory read-modify-write cases keep separate read and write phases. Validation gate: unit and short integration tests cover `imm8`/`imm16` ordering, `(HL)` timing versus register timing, direct and indirect loads, ALU flag behavior for implemented families, and deterministic synthetic ROM execution for those instruction groups.
3. `Phase 2.3` - Control flow, stack traffic, prefixes, and boot-prerequisite opcode closure.
   Acceptance criteria: conditional taken and untaken paths execute through different temporal sequences, stack operations become byte-oriented bus traffic, `CALL`/`RET`/`RST` reuse that same stack model, CB-prefixed execution keeps the double-fetch explicit, and the project records one concrete boot-ROM prerequisite opcode matrix before attempting real boot. Validation gate: focused tests cover taken versus untaken timing for `JR`/`JP`/`CALL`/`RET`, stack byte order and `SP` updates, CB-prefix fetch sequencing, and short deterministic programs that cross branches, stack transfers, and prefixed instructions.
4. `Phase 2.4` - Real boot execution and `FF50` cartridge handoff.
   Acceptance criteria: `RealBoot` starts at `0x0000` on the same CPU core and scheduler used after startup, boot ROM overlay stays bus-owned, boot code reaches cartridge execution only through an executed `FF50` write, the next fetch after that write already comes from cartridge `0x0100`, invalid logo or checksum cases remain in boot, and `No MBC` is the first closed real-boot cartridge baseline. Validation gate: automated tests cover boot-ROM visibility before handoff, next-fetch cartridge visibility after `FF50`, valid handoff versus invalid header non-handoff, and DMG-family cartridge-entry state coming from executed firmware rather than direct-boot literals. Closure note: the synthetic DMG boot ROM from the first `2.4` landing stays as a narrow deterministic `gb-core` target, but production DMG-family boot closure is now defined by ignored verified-boot-ROM regressions that cover `dmg0` / `dmg` / `mgb` valid handoff plus DMG invalid-logo, invalid-checksum, and FF-filled-header non-handoff with private firmware supplied through `GB_CYCLE_BOOT_ROM_ROOT` and without requiring firmware dumps in CI.
5. `Phase 2.5` - Interrupt-controller integration and CPU accept/service flow.
   Acceptance criteria: hardware producers request interrupts through the interrupt controller, `IF` visibility remains separated from CPU acceptance, `IME`, delayed `EI`, immediate `DI`, fixed priority, acknowledge, `RETI`, and the real `20` T-cycle service sequence are all represented explicitly, and scheduler step `8` versus step `9` remains visible in code and traces. Validation gate: focused tests cover `IF`/`IE` MMIO behavior, pending IRQ visibility with `IME = 0`, priority resolution, `EI ; NOP`, `EI ; DI`, `RETI`, and interrupt service timing as a real multi-step CPU sequence. Closure note: this phase closes the interrupt-controller plus CPU contract, including phase-`8` aggregation into `IF`, phase-`9` CPU acceptance, delayed `EI`, immediate `DI`, `RETI`, and bytewise `20` T-cycle servicing. Concrete request-generation rules for timer, PPU, serial, and joypad still land in their owning subsystem phases.
6. `Phase 2.6` - `HALT`, `STOP`, and the `HALT` bug.
   Acceptance criteria: `HALT`, `STOP`, wake-up, and later interrupt service remain distinct ordered events, the `HALT` bug is modeled as a next-fetch effect instead of a generic `PC` shortcut, and DMG `STOP` wake flows through the joypad-owned hardware path rather than a frontend-only resume. Validation gate: focused tests cover `HALT` with `IME = 1`, `HALT` with `IME = 0`, already-pending IRQ plus `HALT`, `HALT` bug fetch behavior, row-selected DMG `STOP` wake, and the ordering between wake and later interrupt acceptance. Closure note: this phase closes the baseline control-state model for `HALT`, `STOP`, wake from joypad-owned input transitions, and a next-fetch `HALT` bug implementation on the shared scheduler timeline, including the explicit DMG `STOP` entry matrix for real-stop, zombie-stop, HALT-like, and NOP-like behavior under `IME = 0`, plus the simpler `IME = 1` entry rule where `WAKE = 1` collapses to a one-byte NOP-like path and `WAKE = 0` enters the ordinary two-byte-visible stop. The same baseline now also includes one explicit deterministic model for the documented `IME = 1` wakeup-time joypad IRQ glitch family: a bugged interrupt-service window to `0x0000` plus a corrupted push caused by losing the final stack-side decrement during that service. The `IME = 0`, `WAKE = 0`, pending-IRQ zombie-stop branch is also explicit now: it appears as a 1-byte `STOP` sleep state that resumes CPU fetch from `PC + 1` on a later joypad-owned wake event, while the repo still treats the reported "high power" aspect as non-software-visible and therefore not as a separate shared-clock mode.
7. `Phase 2.7` - Timer edge model, overflow pipeline, and delayed timer IRQ.
   Acceptance criteria: timer state is driven by the shared internal divider on the global T-cycle timeline, `DIV` stays a derived view of the internal counter, `TAC` selection and enable feed falling-edge TIMA increments, overflow enters an explicit delayed reload/request pipeline, timer requests become visible in `IF` only after that delay, and `SkipBoot` synthesizes timer hidden state coherently with the visible post-boot snapshot. Validation gate: focused tests cover `DIV` reset behavior, `DIV` and `TAC` glitch cases, frequency-selection edge timing, TIMA overflow and reload windows, delayed timer request visibility, and timer-plus-interrupt integration without flattening request and service into one instant event. Closure note: this phase closes the timer baseline around the shared `system_counter`, falling-edge TIMA increments, `4` T-cycle delayed reload and request, plus CPU-visible integration with `IF` and later interrupt service ordering.
8. `Phase 2.8` - Phase closure, regression matrix, and oracle cross-check.
   Acceptance criteria: tracing can show opcode fetches, operand accesses, `IF` visibility, interrupt acceptance, and boot handoff on one shared timeline, Phase 2 local TODOs are either closed or explicitly documented, and the resulting CPU/timer/IRQ/boot stack is stable enough to stop being a moving target for later DMA and PPU work. Validation gate: the full unit and integration suite passes, the first Phase 2 ROM automation targets land for CPU and interrupt timing, and timing-sensitive divergences are cross-checked against SameBoy before the phase is considered closed. Closure note: this phase closes with one shared trace timeline exposing phase-`5` CPU bus activity (`opcode_fetch`, `operand_read`, `data_read`, `data_write`), phase-`8` `IF` visibility, phase-`9` post-acceptance CPU and interrupt state, plus phase-`6` boot handoff visibility around `FF50`. The first Phase `2` ROM automation targets now exist as typed `gb-test-runner` suites for CPU and interrupt timing. Remaining local TODOs stay explicit under the Phase `2` section below, and the retained SameBoy source-level comparison is now summarized in [`docs/REFERENCES.md`](../REFERENCES.md); automated first-divergence tooling is no longer an active gb-cycle repo-local deliverable.

#### Risks if done late or superficially

- inability to model HALT bug correctly
- incorrect interrupt acceptance
- timer that appears correct but is temporally false
- the need to rework much of the core when integrating PPU, DMA, or demanding test ROMs
