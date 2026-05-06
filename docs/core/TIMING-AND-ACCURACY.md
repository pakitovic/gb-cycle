# Timing and Accuracy

## Accuracy policy

This project aims for hardware-faithful behavior, with special care for timing-sensitive subsystems.

## Important distinction

Do not use "cycle accurate" loosely. Track accuracy by subsystem:

- CPU execution timing
- bus access timing
- timer edge behavior
- interrupt ordering
- PPU mode timing
- pixel fetch and FIFO timing
- DMA timing
- APU sequencing and output timing

## Practical rule

A subsystem should only be described as cycle accurate when there is evidence for that claim.

## Sources of confidence

Use this order:

1. hardware documentation and research
2. test ROM behavior
3. comparison with trusted emulators
4. project-local assumptions

## Development policy

When implementing timing:

- document the expected ordering of events
- document the level of confidence
- state which source supports the model
- avoid claiming global cycle accuracy from local evidence

## Closure and validation claims

- Do not treat "boots a few games" as evidence that DMG timing is closed.
- Do not treat one matching final framebuffer as sufficient evidence for a timing-sensitive subsystem when the path to that output is still unverified.
- Closing a timing-sensitive DMG subsystem should rest on layered evidence: focused automated tests, ROM-based validation, trusted-oracle comparison, and determinism checks.
- Because this project is T-cycle based, the validation stack should be able to compare behavior at end-of-test, end-of-instruction, and short per-T-cycle windows when locating the first divergence matters.
- If coarser summaries are used for ergonomics, keep a path back to the underlying T-cycle ordering rather than hiding it behind aggregate instruction or frame-level claims.
- Trace, comparison, and debugging instrumentation must preserve the same hardware-visible T-cycle behavior they are trying to explain.

## Modeling policy

- Prefer explicit clock ownership.
- Make temporal edges visible in code and tests.
- Do not replace hardware ordering with convenience batching unless the behavior is proven equivalent for the target accuracy.
- Favor clarity and testability over shortcuts that obscure the phase model.
- Treat any local simplification that contradicts the T-cycle model or dot-by-dot PPU as explicit documented technical debt, not as a normalized convenience.
- When implementation ease conflicts with temporal fidelity, preserve temporal fidelity as long as the resulting model remains maintainable and observable.
- Use T-cycles as the fundamental execution unit for the core timing model.
- Use dot or T-cycle level reasoning as the baseline timing vocabulary for the core.
- The shared T-cycle timeline governs powered-on hardware execution; it does not imply that battery-backed off-session progression, such as `MBC3` RTC advance while the console is off, is derived from CPU T-cycles.
- Treat M-cycles only as a descriptive grouping of four T-cycles, not as the project's primary timing abstraction.
- When external documentation expresses a timing rule in M-cycles or microseconds, restate the corresponding T-cycle value in project docs and code whenever that rule becomes behaviorally relevant or is recorded as deferred validation work.
- Keep the timing model clean enough that future CGB double-speed behavior can be expressed as an extension of the same temporal foundation rather than a separate clocking design.

## Global scheduler rule

- The core should advance through one fixed per-T-cycle scheduler contract, not through ad hoc call chains between subsystems.
- The recommended ordering is:
  1. external event ingress
  2. master clock / shared system-counter tick
  3. free-running counter-derived edge resolution
  4. autonomous peripheral ticks
  5. bus arbitration
  6. CPU micro-operation
  7. MMIO side-effect commit
  8. interrupt aggregation into `IF`
  9. CPU wake / interrupt-accept evaluation
- This is a project-level deterministic ordering rule chosen to preserve documented dependencies; it is not presented as the one true internal Nintendo implementation.
- Free-running divider-derived events such as timer input edges and `DIV-APU` edge detection belong to step `3`, after the shared counter advances and before autonomous peripherals consume those edges.
- Immediate MMIO effects produced by a write on the access T-cycle, such as `DIV` reset behavior, `FF46` DMA start, `SC.7` transfer start, or `LCDC.7` LCD transitions, still belong to the owning device when the access commits in step `7`.
- In the current March 27, 2026 DMG baseline, the shared scheduler now stages CPU-originated PPU MMIO writes during step `6` and commits them during step `7`. That keeps the runtime aligned with the documented phase contract without changing the existing DMG `Mode 3` rule that active-pipeline register snapshots only become visible on the next PPU dot after the commit.
- `IF` updates from hardware sources belong to step `8`; CPU acceptance is a later CPU-owned decision and must not be collapsed into the producer path, except that Timer reload requests already queued before CPU step `6` may preempt the next opcode slot and are consumed so step `8` does not reassert an already accepted Timer source.
- Another internal implementation shape is acceptable only if these same observable dependencies remain true.

## Practical timing rule

- CPU, PPU, timer, APU, DMA, and bus-visible effects should be expressible on a shared T-cycle timeline.
- Avoid architectures that execute a whole instruction or whole M-cycle and only then advance the rest of the hardware.
- If higher-level helpers exist for ergonomics, they must preserve the same per-T-cycle ordering semantics internally.
- For the CPU specifically, opcode fetch, immediate fetch, stack transfer, indirect memory access, conditional timing splits, and internal no-bus steps should all remain expressible as ordered events on that timeline.
- Long-running hardware activity started by a register write, such as OAM DMA, should become explicit in-flight state on the shared timeline rather than a one-shot side effect.
- When a subsystem progresses in repeated temporal slices, such as one DMA byte every four dots on DMG OAM DMA, that phase relationship should remain visible in code and tests.
- The timing model must support both fixed-duration burst transfers and windowed or block transfers that only advance in eligible windows such as HBlank, while keeping both on the same shared T-cycle timeline.
- A transfer's CPU or bus impact and its data-copy action should be expressible separately per T-cycle, because arbitration may be visible on cycles where no byte commits and future block DMA may stall only for selected windows.
- Future transfer advance may depend on other live machine state such as PPU HBlank visibility or CPU `HALT` state; keep that dependency explicit in the transfer model rather than hiding it in bus code.
- Edge-driven subsystems such as the timer should derive visible events from clocked internal state transitions, not from coarse accumulated-period shortcuts.
- CPU interrupt acceptance, delayed `EI`, `HALT` wake-up, and `HALT` bug behavior should be expressed as ordered events on that same shared timeline rather than as opaque instruction-level shortcuts.
- For the PPU specifically, dot-by-dot progression is the intended interpretation of the shared T-cycle timeline.
- MMIO reads and writes should also be modeled as ordered T-cycle events on that same timeline, not as timeless getters/setters attached to instruction completion.
- Read or write side effects triggered by MMIO, such as `DIV` reset, `LCDC.7` LCD enable changes, `FF46` DMA start, `FF50` boot-ROM unmapping, `SC.7` transfer control, or `NRx4` channel triggers, should occur on the access T-cycle unless hardware evidence says otherwise.
- Public setup/debug helpers such as `Machine::write_bus()` are outside that shared scheduler timeline and may still apply owner-visible MMIO state immediately; do not use those helpers as evidence that the runtime CPU path has no phase-`7` MMIO commit boundary.
- Reads of dynamic MMIO state such as `LY`, `STAT` mode bits, interrupt flags, in-progress serial state, or APU channel-status bits should observe the live hardware state at the instant of the read.
- Host-side persistence work such as save flushes, timestamp capture, or atomic file replacement is outside the emulated T-cycle timeline; it must not be used to reorder or retroactively redefine already-committed hardware-visible cartridge state.
- `JOYP` should follow that same MMIO rule: `FF00` selection writes take effect on the access T-cycle, and later reads should observe the currently selected rows and current hardware-facing button state at the instant of the read.
- `SB` and `SC` should follow that same MMIO rule: writes change serial state on the access T-cycle, and reads during transfer should be able to observe live serial progress rather than a deferred final-byte snapshot.
- If hardware truly defers an MMIO-visible effect, model that deferral explicitly as timed state rather than as an informal "apply MMIO side effects later" queue.
- Host input should enter the core as changes to abstract button state on the shared timeline, not as end-of-frame batches that overwrite the final visible `JOYP` value.
- Joypad interrupt requests should be derived from `High -> Low` transitions in the visible `P1` low nibble after row selection has been resolved, not from raw host input events detached from `JOYP` visibility.
- If input timing affects CPU `STOP` wake-up, that wake path should preserve the same ordered T-cycle relationship between button-state change, any resulting `JOYP` visibility change, any resulting interrupt request, and the CPU state transition.
- DMG master-mode serial clocking should derive from the shared machine timeline at the documented `8192` Hz rate rather than from host sleep or wall-clock timers.
- Slave-mode serial clock pulses should be injectible on the shared timeline with arbitrary spacing, and serial completion plus IRQ request should occur on the exact pulse that completes the eighth shift.
- For the APU specifically, internal channel state, `DIV-APU`, frame-sequencer phase, mixer-visible DAC state, and HPF state should advance on that shared T-cycle timeline rather than on host audio callback cadence.
- The APU frame sequencer should derive its slow control clock from the same shared divider timeline as `DIV`; for the current DMG target, that means reacting to the falling edge of `DIV` bit `4`, including `DIV`-write-induced extra ticks when the edge occurs.
- Slow APU control clocks such as length, envelope, and CH1 sweep must remain distinct from each channel's own fast waveform timer and from the host sample or resampler cadence.
- Host-rate sample production should observe already-stepped hardware state; it must not become the clock that drives the APU core.
- When cartridge hardware uses wall-clock-like progression outside powered-on execution, such as battery-backed `MBC3` RTC advance between sessions, model that through an explicit elapsed-time source at the persistence boundary and restate any powered-on bus-visible timing rule in T-cycles when it becomes behaviorally relevant.
- Timer, joypad, serial, and the APU frame sequencer must not be tied to video-frame or VBlank callbacks; they live on the shared machine timeline even when their visible effects are unrelated to the LCD.
- Bus restrictions and MMIO-visible state must tell one coherent story on that timeline. For example, `STAT.mode`, VRAM/OAM accessibility, DMA blocking, `SC.7`, `TIMA/TMA/IF`, and visible `JOYP` state must align with the same current-cycle machine state the scheduler uses internally.
