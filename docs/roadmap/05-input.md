# Phase 5 — Input and simple peripherals

23. **Joypad/input and its relation to interrupts**
24. **Serial port**

#### Goal

Complete basic system peripherals on top of an already consolidated bus, scheduler, and interrupt model.

#### Modules involved

- `joypad/`
- `serial/`
- `bus/`
- `scheduler/`
- `cpu/`
- `debugger/`

#### Deliverables

- joypad register reads/writes
- `JOYP` implemented as a mixed register at `FF00`, with latched row-selection bits and a dynamic active-low low nibble derived from a `2x4` button matrix
- explicit separation between frontend-provided abstract button state and emulated joypad/MMIO state
- visible-edge detection for joypad interrupt generation based on the low nibble actually exposed through `P1`
- input-driven `STOP` wake integration routed through the same joypad subsystem rather than a frontend bypass
- interrupt generation where appropriate
- `SB` and `SC` implemented as serial-owned MMIO state, with serial transfer modeled as a bit-level process rather than an instant byte exchange
- explicit serial-peer boundary for disconnected, loopback, scripted, and future real link peers
- serial interrupt generation driven by real transfer completion rather than by transfer start
- serial port functional at the emulated hardware level
- trace tools to observe both peripherals
- scheduler-visible regression coverage for their IRQ timing and CPU wake interactions

#### Done criteria

- joypad and serial are decoupled from the frontend
- writing `0x30` to `JOYP` makes the visible low nibble read back as `0xF`
- `JOYP` bits `7-6` read back high in the current DMG-family baseline instead of mirroring arbitrary storage
- selecting only one joypad row exposes only that row's buttons in the visible low nibble
- selecting both joypad rows follows one explicit combined-matrix rule for both readback and interrupt detection rather than an invented row priority
- joypad interrupt requests are generated only by visible `High -> Low` transitions on `P1` bits `0-3`, and only when the relevant row selection makes that transition visible
- repeated visible input transitions can request joypad interrupts repeatedly; the model does not assume one interrupt per press
- `STOP` wake-up is driven from the same joypad/input subsystem path used for hardware-visible input state, not by directly toggling CPU state from the frontend
- for the current repo DMG-family baseline, `STOP` wake is a selection-independent `released -> pressed` transition on any hardware-facing button, kept distinct from joypad-interrupt visibility rules
- `SB` changes progressively during active serial transfer instead of remaining frozen until the final byte arrives
- in DMG master mode, serial transfer advances through `8` internally generated clocks at `8192` Hz and clears `SC.7` only on completion
- in slave mode, serial transfer does not advance without externally injected clocks and does not time out internally
- disconnected serial-peer behavior is explicit and tends toward receiving `0xFF`
- serial interrupt requests occur only at transfer completion, when the eighth shift clears `SC.7`
- both are integrated through the bus and scheduler
- their interrupts and states are observable and testable
- their event timing does not depend on frame callbacks or host timers bypassing the T-cycle scheduler

#### Recommended sequencing inside Phase 5

Phase `5` should be executed as narrow subphases. No subphase counts as closed
unless its local acceptance criteria land together with focused automated
coverage and preserve the existing scheduler, bus, and interrupt-controller
contracts instead of reopening them through peripheral-local shortcuts.

1. `Phase 5.1` - Joypad register closure and hardware-facing input boundary.
   Acceptance criteria: `JOYP` remains joypad-owned as a mixed register, the
   frontend-facing API only updates hardware-facing button state instead of
   precomposed `FF00` bytes, bits `7-6` read back high, `0x30` reads back with
   low nibble `0xF`, and selecting one or both rows resolves the low nibble
   from one explicit `2x4` matrix rule rather than from row-priority shortcuts.
   Validation gate: focused unit and MMIO integration tests cover row
   selection, active-low semantics, simultaneous-row combination, direct-boot
   startup state, and the guarantee that selection writes affect readback on
   the same shared machine timeline.
   Status: done in the current branch baseline for both the core boundary and
   the desktop host adapter. `gb-desktop` now aggregates keyboard plus SDL3
   gamepad state on the host side, including active-pad hotplug handoff, while
   only forwarding effective `JoypadButton` transitions into `gb-core` instead
   of reintroducing frontend-owned `JOYP` composition.
2. `Phase 5.2` - Joypad visible-edge interrupt generation through the shared
   interrupt path.
   Acceptance criteria: joypad tracks the previously visible low nibble, raises
   a request only on visible `High -> Low` transitions after row selection is
   applied, repeated visible transitions can request multiple interrupts, and
   the request enters `IF` only through the shared interrupt controller.
   Validation gate: focused unit and integration tests cover selected-row,
   unselected-row, both-rows-selected, and selection-write-created edge cases,
   plus machine-level verification that `IF` changes only when the visible
   `JOYP` low nibble actually transitions.
   Status: done in the current branch baseline. `Joypad` now owns previous
   visible-low-nibble tracking, both `FF00` selection writes and hardware-side
   button transitions feed the same edge detector, and the resulting request is
   drained into `IF` only during scheduler phase `8` aggregation rather than by
   direct `FF00` or frontend-side mutation of the interrupt controller.
3. `Phase 5.3` - Joypad-driven `STOP` wake closure on the shared scheduler
   timeline.
   Acceptance criteria: the repo's current DMG-family `STOP` wake policy
   remains explicit as selection-independent `released -> pressed` wake on any
   hardware-facing button, that wake originates from the joypad subsystem path
   rather than from frontend or CPU bypasses, and wake ordering stays distinct
   from joypad-interrupt generation even when both happen around the same input
   change.
   Validation gate: focused CPU/joypad integration tests cover `STOP` wake with
   no visible joypad IRQ, `STOP` wake plus later interrupt servicing, repeated
   wake-producing input transitions, and one negative case proving that a
   non-transition or already-held button does not produce an extra wake event.
   Status: done in the current branch baseline. `STOP` wake continues to come
   only from the joypad-owned released-to-pressed path, remains selection
   independent across the `8` hardware-facing buttons, and stays temporally
   distinct from any same-input-change joypad interrupt request or later CPU
   interrupt service.
4. `Phase 5.4` - Serial MMIO closure and explicit transfer-state baseline.
   Acceptance criteria: `SB` and `SC` stay serial-owned, `SC.7` means
   transfer-requested or in-progress rather than instant completion, DMG
   non-functional bits still read high, the serial subsystem exposes one
   explicit in-flight transfer shape with bit count and clock-source state, and
   startup-state injection continues to come from the centralized boot path.
   Validation gate: focused unit and MMIO integration tests cover `SB` / `SC`
   readback, transfer arming without instant completion, internal versus
   external clock selection, direct-boot startup state, and snapshot/debug
   visibility of the new transfer state.
   Status: done in the current branch baseline. `SB` / `SC` remain
   serial-owned, `SC.7` still means transfer requested rather than completed,
   and the serial snapshot/debug surface now exposes one explicit pending
   transfer shape with selected clock mode plus `bits_shifted = 0` ahead of the
   later bit-level engine work in `Phase 5.5`.
5. `Phase 5.5` - Bit-level serial engine, peer boundary, and completion-driven
   IRQ timing.
   Acceptance criteria: DMG master mode advances one serial shift per internal
   clock pulse at `8192` Hz on the T-cycle timeline, slave mode does not
   advance without externally injected clocks, disconnected peers yield incoming
   `1` bits tending toward `0xFF`, `SB` evolves during transfer rather than
   jumping at the end, and the serial interrupt is requested only when the
   eighth shift clears `SC.7`.
   Validation gate: focused unit and integration tests cover intermediate `SB`
   states, master-mode timing, slave-mode pending state, disconnected-peer
   behavior, one loopback or scripted-peer case, and the same-cycle coherence
   of final `SB`, cleared `SC.7`, and serial `IF` request on transfer
   completion.
   Status: done in the current branch baseline. DMG master mode now shifts one
   bit every `512` T-cycles (`8192` Hz), slave mode remains pending without
   externally queued clocks, disconnected input tends toward `0xFF`, loopback
   is explicit through the serial peer boundary, and completion clears `SC.7`
   while requesting the serial interrupt in the same scheduler-visible cycle.
6. `Phase 5.6` - Traceability, regression assets, and phase closure.
   Acceptance criteria: scheduler-visible traces expose joypad selection/input
   edges, joypad IRQ requests, `STOP` wake eligibility, serial start/progress /
   completion, and peer-driven external-clock events; the phase closes only
   once the resulting peripheral behavior is covered by targeted unit tests,
   subsystem integration tests, and retained artifacts where timing visibility
   matters.
   Validation gate: phase-level regression tests retain at least one
   joypad-and-`STOP` timing artifact and one serial timing artifact, and any
   timing-sensitive open question is either cross-checked against a trusted
   oracle or recorded immediately as a roadmap TODO instead of being carried
   informally.
   Status: done in the current branch baseline. Scheduler-visible traces now
   expose joypad state during interrupt aggregation and CPU wake evaluation,
   serial progress during autonomous-peripheral ticks, and retained Phase `5`
   trace fixtures now lock one joypad-plus-`STOP` chronology and one
   peer-driven external-clock serial chronology without introducing any new
   timer-driven open question that would force the deferred Phase `2.7`
   `TIMA` / `TMA` arbitration work into this phase.

#### Joypad implementation breakdown

1. **`JOYP` mixed-register baseline**
   Scope: `FF00` row selection, active-low low-nibble readback, and read/write ownership in `joypad/`.
   Acceptance criteria: `0x30` reads back with low nibble `0xF`, `JOYP` bits `7-6` stay high, selecting buttons versus directions changes which row is visible, and the frontend does not write precomposed `JOYP` bytes directly.
2. **Internal button-matrix state**
   Scope: one hardware-facing state model for all `8` buttons, separated from frontend host input details.
   Acceptance criteria: any button can be pressed or released without touching MMIO directly, and `JOYP` readback derives from that state plus current row selection.
3. **Joypad interrupt generation**
   Scope: visible `High -> Low` detection on `P1` low bits and request routing into `IF`.
   Acceptance criteria: the interrupt appears only when the relevant row is selected; multiple visible transitions can request multiple interrupts; joypad does not bypass the shared interrupt controller.
4. **`STOP` integration**
   Scope: route input-driven wake behavior through the joypad subsystem and CPU `STOP` state interface.
   Acceptance criteria: a `released -> pressed` transition on any hardware-facing button can wake `STOP` regardless of current `JOYP` row selection, and that wake path does not bypass the joypad subsystem.
5. **Focused validation**
   Scope: matrix selection, active-low semantics, simultaneous-row selection, visible-edge IRQ detection, and `STOP` wake behavior.
   Acceptance criteria: tests cover buttons and d-pad separately, both rows selected, visible `High -> Low` detection, repeated input transitions, and the documented repo policy that `STOP` wake is selection-independent while joypad IRQ generation is still selection-dependent.

#### Serial implementation breakdown

1. **`SB` / `SC` MMIO baseline**
   Scope: `FF01`, `FF02`, ownership in `serial/`, DMG control-bit semantics, and non-functional `SC.1` reservation for future CGB work.
   Acceptance criteria: `SB` and `SC` have clear serial ownership, `SC.7` means requested-or-in-progress transfer, `SC.0` selects internal versus external clock, DMG does not expose functional high-speed serial through `SC.1`, and the other non-functional DMG `SC` bits read back high through the routed MMIO contract.
2. **Bit-level master transfer**
   Scope: DMG internal-clock master mode, `8` serial shifts, live `SB` evolution, and completion-driven `SC.7` clear plus IRQ.
   Acceptance criteria: `SB` changes progressively during transfer, the DMG internal clock runs at `8192` Hz on the machine timeline, and transfer completion requests the serial interrupt only after the eighth shift.
3. **Peer boundary and disconnected behavior**
   Scope: explicit serial-peer interface, disconnected input policy, loopback, and scripted peers.
   Acceptance criteria: the core works without a real link peer, disconnected input yields incoming `1` bits and tends toward `0xFF`, and loopback or scripted peers can be attached without direct MMIO byte injection.
4. **Slave mode with external clock**
   Scope: externally driven serial clocks, pending transfer state, and non-uniform pulse timing.
   Acceptance criteria: arming slave mode does not advance transfer on its own, each externally injected clock performs one shift, and the transfer completes only on the eighth external pulse.
5. **Interrupt and scheduler closure**
   Scope: full `SB` / `SC` -> transfer -> `IF` route plus timing-visible reads and writes.
   Acceptance criteria: `IF` receives the serial request at the correct completion point, `SC.7` clears at that same point, and tests cover master mode, slave mode, disconnected peer, loopback or scripted peer, and intermediate `SB` states.

#### Phase 5 interleave policy with earlier open TODOs

- Phase `3` and Phase `5`'s own section currently leave no open TODOs, so DMA and cartridge work are not sequencing blockers for entering the input/peripheral phase.
- The resolved Phase `2.6` `EI ; HALT` pending-IRQ edge no longer blocks `Phase 5.3`; keep using that path as a regression target when extending joypad-driven wake and interrupt coverage so later refactors do not silently reopen it.
- The remaining Phase `2` exact reload-cycle `TIMA` / `TMA` arbitration is still deferred and should stay isolated unless a serial-completion or joypad-interrupt test proves that the shared interrupt timeline is modeled incorrectly for reasons broader than the timer itself.
- The remaining Phase `4` TODOs are validation-grade PPU follow-ups, not architectural blockers for Phase `5`; only interleave one of them if shared scheduler traces, oracle tooling, or retained artifact plumbing can be improved once and reused immediately by the active joypad or serial subphase.
- If a Phase `5` subphase depends on a missing helper, fixture pattern, or trace hook that also resolves a concrete earlier TODO, land that smallest reusable seam first instead of duplicating temporary peripheral-local scaffolding.
- If a Phase `5` subphase lands with a deliberately isolated gap, record the remainder in `Open TODOs` immediately instead of carrying it informally into later cartridge or APU work.

#### Subphase exit rule

Every Phase `5` subphase should end with:

- focused unit tests for the local register contract, edge detector, transfer state machine, or peer boundary that was introduced
- integration tests when the behavior only becomes meaningful across `joypad`, `serial`, `cpu`, `interrupts`, `bus`, or `machine`
- retained trace or snapshot coverage when timing visibility, `STOP` wake ordering, or serial progress would otherwise be hard to audit after a refactor
- `cargo test -q` passing locally at minimum, and `make ci` whenever the subphase changes shared validation/tooling or other workflow-critical infrastructure
- at least one explicit note about remaining risk when oracle comparison or external-ROM validation is still intentionally deferred
- a roadmap TODO recorded immediately if the subphase ships with a concrete uncovered gap

