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

## Modeling policy

- Prefer explicit clock ownership.
- Make temporal edges visible in code and tests.
- Do not replace hardware ordering with convenience batching unless the behavior is proven equivalent for the target accuracy.
- Favor clarity and testability over shortcuts that obscure the phase model.
- Use T-cycles as the fundamental execution unit for the core timing model.
- Use dot or T-cycle level reasoning as the baseline timing vocabulary for the core.
- Treat M-cycles only as a descriptive grouping of four T-cycles, not as the project's primary timing abstraction.
- Keep the timing model clean enough that future CGB double-speed behavior can be expressed as an extension of the same temporal foundation rather than a separate clocking design.

## Practical timing rule

- CPU, PPU, timer, APU, DMA, and bus-visible effects should be expressible on a shared T-cycle timeline.
- Avoid architectures that execute a whole instruction or whole M-cycle and only then advance the rest of the hardware.
- If higher-level helpers exist for ergonomics, they must preserve the same per-T-cycle ordering semantics internally.
- For the CPU specifically, opcode fetch, immediate fetch, stack transfer, indirect memory access, conditional timing splits, and internal no-bus steps should all remain expressible as ordered events on that timeline.
- Long-running hardware activity started by a register write, such as OAM DMA, should become explicit in-flight state on the shared timeline rather than a one-shot side effect.
- When a subsystem progresses in repeated temporal slices, such as one DMA byte every four dots on DMG OAM DMA, that phase relationship should remain visible in code and tests.
- Edge-driven subsystems such as the timer should derive visible events from clocked internal state transitions, not from coarse accumulated-period shortcuts.
- CPU interrupt acceptance, delayed `EI`, `HALT` wake-up, and `HALT` bug behavior should be expressed as ordered events on that same shared timeline rather than as opaque instruction-level shortcuts.
- For the PPU specifically, dot-by-dot progression is the intended interpretation of the shared T-cycle timeline.
- MMIO reads and writes should also be modeled as ordered T-cycle events on that same timeline, not as timeless getters/setters attached to instruction completion.
- Read or write side effects triggered by MMIO, such as `DIV` reset, `LCDC.7` LCD enable changes, `FF46` DMA start, `FF50` boot-ROM unmapping, `SC.7` transfer control, or `NRx4` channel triggers, should occur on the access T-cycle unless hardware evidence says otherwise.
- Reads of dynamic MMIO state such as `LY`, `STAT` mode bits, interrupt flags, in-progress serial state, or APU channel-status bits should observe the live hardware state at the instant of the read.
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
