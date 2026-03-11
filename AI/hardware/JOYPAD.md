# JOYPAD

## Scope

Own the joypad register view, the underlying `2x4` button matrix model, button selection lines, interrupt request signaling, and input-driven wake signaling that must remain tied to the same hardware-visible state.

## Hardware model

Keep the hardware-visible register behavior separate from host input collection.
Model the joypad as a matrix peripheral rather than as "a byte the frontend writes into `FF00`".
Keep these layers distinct:

- physical or hardware-facing state of the `8` buttons
- writable row-selection state in `P1/JOYP`
- CPU-visible low-nibble readback derived from the currently selected rows
- edge detection derived from the visible low nibble, not directly from host input events

## Responsibilities

- `P1/JOYP` register behavior
- `2x4` button matrix ownership
- button matrix selection handling
- visible low-nibble composition from selected rows
- previous-visible-state tracking or equivalent edge detection
- interrupt signaling on visible input transitions
- wake signaling for CPU `STOP` integration through the same input path

## Registers / MMIO

- `JOYP` at `FF00`

## `JOYP` contract baseline

- `JOYP` should be implemented as a mixed register, not as a flat stored byte.
- Bits `5` and `4` are the writable row-selection lines: bit `5` selects the button row when written as `0`, and bit `4` selects the d-pad row when written as `0`.
- The selection bits belong to the register's writable state, while the low input nibble is read-only and derived from the current button matrix state.
- The low nibble is active-low: a pressed button reads back as `0`.
- If neither button row nor d-pad row is selected, the low nibble should read back as all released, `0xF`.
- The visible low nibble must never be treated as "the last byte written to `FF00`"; it is a live view resolved from the selected rows and the current button matrix state.
- If both rows are selected at once, readback should follow the hardware-style simultaneous matrix observation path where a visible bit reads low whenever any selected row pulls that shared line low; do not invent a software-friendly priority between buttons and directions.
- `JOYP` reads should be side-effect free unless later hardware evidence proves otherwise; software that reads repeatedly to stabilize input should see the current matrix state rather than a frontend-written echo.
- The frontend/input adapter should update a hardware-facing button state, not write bytes directly into `JOYP`.

## Joypad interrupt baseline

- The joypad interrupt request condition is a `High -> Low` transition on any visible `JOYP` bit in the low nibble after row selection has been applied.
- Interrupt generation must therefore compare the previous visible low nibble against the newly visible low nibble, or use an equivalent explicit edge detector.
- A physical button-state change on an unselected row must not request the joypad interrupt until that change becomes visible through the current `JOYP` selection state.
- If both rows are selected, interrupt detection must use the same combined visible-nibble semantics as readback rather than a second simplified path.
- Do not collapse the logic into "one press equals one interrupt". Multiple visible `High -> Low` transitions may request the joypad interrupt multiple times if input changes or bounce-like test stimuli produce repeated edges.
- Joypad should request the interrupt through the shared interrupt-controller path, not by dispatching CPU interrupt service directly.

## `STOP` integration baseline

- The joypad subsystem should be the hardware-facing origin of input-driven CPU wake signaling relevant to `STOP`, rather than letting the frontend or UI wake the CPU by bypassing emulated hardware state.
- The wake path should derive from the same joypad-owned hardware-facing button state and be documented here as the repo's DMG-family `STOP` wake policy, rather than being inferred from a frontend callback or redefined inside the CPU.
- If the exact DMG-family electrical wake condition remains under research, that uncertainty should still be expressed here as one explicit repo policy or open question; other docs should not invent a second rule.
- `STOP` wake handling and joypad interrupt generation are related but not identical concerns; keep them explicitly connected through shared joypad state without merging them into one opaque shortcut.

## Timing / accuracy requirements

- Preserve hardware-visible register semantics even if host input arrives asynchronously.
- `JOYP` selection writes at `FF00` should take effect on the access T-cycle, not at the end of a frontend frame or instruction batch.
- `JOYP` reads should observe the current selected rows and current input state at the instant of the MMIO read.
- If host input changes between two MMIO accesses or other scheduler-visible points, the next `JOYP` read should observe that new matrix state according to the current selection lines.
- Joypad interrupt detection should run on the shared T-cycle timeline using the visible `JOYP` low nibble before and after the relevant state change.
- Repeated reads of `JOYP` must remain live reads; do not add bus-side or CPU-side caching that would flatten software-visible stabilization loops.
- Host input ingestion may be event-driven or latched between core ticks, but `JOYP` readback and interrupt generation should still resolve from a hardware-facing button state on the shared core timeline rather than from a frontend frame callback.
- The joypad subsystem does not need a free-running dot generator of its own, but it must still integrate with the shared scheduler strongly enough that `FF00` writes, `FF00` reads, visible-edge detection, interrupt requests, and `STOP` wake events preserve ordering on the T-cycle timeline.

## Dependencies

- bus/MMIO wiring
- interrupt controller
- CPU `STOP` state integration
- scheduler or shared clock timeline
- frontend input adapter boundary

## Primary references

- Pan Docs joypad sections

## Open-source emulator references

- SameBoy
- binjgb
- GameRoy

## Tests

- register behavior tests
- interrupt signaling tests
- separate tests for button-row selection and d-pad-row selection
- active-low readback tests
- tests that `0x30` selection reads the low nibble back as `0xF`
- tests where both rows are selected and the visible nibble follows the same combined-matrix semantics used by interrupt detection
- tests that joypad interrupt requests come only from visible `High -> Low` transitions in low-nibble bits
- tests that a button change on an unselected row does not request the interrupt until it becomes visible
- tests that repeated input transitions can request the interrupt repeatedly rather than being collapsed to one request per press
- tests that interrupt generation is driven from the same underlying input-state transitions observed through `JOYP`
- tests that the documented repo `STOP` wake policy uses the joypad subsystem path rather than a frontend-only shortcut

## Implementation notes for this repo

- Keep host key mapping outside the emulation core.
- Let bus/MMIO wiring expose `JOYP` at its mapped address while the joypad subsystem owns the register semantics.
- Keep one explicit joypad-owned source of truth for:
  - physical button state
  - latched `JOYP` selection bits
  - current visible low nibble
  - previous visible low nibble or equivalent edge-detection state
- Request the joypad interrupt through the shared interrupt-controller path instead of mutating CPU interrupt state directly.
- Feed any input-driven `STOP` wake path from the same joypad-owned state transition logic rather than from a frontend callback that bypasses emulated hardware.
- Direct-boot startup values such as the documented post-boot `P1` snapshot should be injected through the centralized boot-state path rather than hard-coded as a local joypad reset default.

## Known pitfalls

- mixing frontend input API details into joypad logic
- detecting interrupts from abstract "button pressed" events instead of from visible `P1` transitions
- flattening simultaneous row selection into an invented row priority
- letting CPU or frontend code own `JOYP`-visible state instead of the joypad subsystem
- encoding one `STOP` wake rule in joypad and a different one in CPU or frontend glue

## Open questions

- what frontend-facing event or latch API best feeds button-state changes into the core without hiding the resulting hardware-visible state behind frontend frame timing
