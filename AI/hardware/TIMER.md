# TIMER

## Scope

Own `DIV`, `TIMA`, `TMA`, `TAC`, their internal timing state, overflow behavior, and interrupt request generation.

## Hardware model

Model the timer as edge-sensitive hardware, not as a periodic software counter incremented every few instructions.
The source of truth should be an internal `16`-bit system counter advanced by the shared master clock, with `DIV` and TIMA-driving events derived from that counter rather than maintained as unrelated software counters.

## Responsibilities

- track the internal timer system counter
- expose `DIV` as a derived visible register view
- implement timer enable/frequency selection behavior
- detect the effective timer signal and its relevant edges
- handle overflow, reload, and interrupt request ordering
- integrate writes to `DIV`, `TIMA`, `TMA`, and `TAC` with the timer's internal temporal state

## Registers / MMIO

- `DIV`
- `TIMA`
- `TMA`
- `TAC`

## DMG timer baseline

- The timer should maintain an internal `16`-bit system counter or equivalent state advanced by `1` on every T-cycle.
- `DIV` should be treated as a visible derivation of that internal counter, not as an independent master counter.
- Writing to `DIV` should reset the internal divider/system-counter state rather than storing the written byte literally.
- TIMA increments should come from a falling-edge (`1 -> 0`) detection on the effective timer signal, not from a generic "every N cycles" accumulator.
- The effective timer signal on DMG is `timer_enable && selected_counter_bit`.
- The TAC frequency selection should be modeled as internal counter-bit selection, using the DMG mapping:
  - `00 -> bit 9`
  - `01 -> bit 3`
  - `10 -> bit 5`
  - `11 -> bit 7`
- Timer overflow should be modeled as a temporal process with explicit pending/reload state; do not collapse overflow, reload from `TMA`, and interrupt request into one instant write-like event.
- On DMG, the timer interrupt request does not become visible at the same logical moment as overflow detection. The `TMA` reload and timer request into `IF` arrive one M-cycle later.

## MMIO contract baseline

- `DIV`, `TIMA`, `TMA`, and `TAC` belong to the timer subsystem; MMIO is only the external contract by which other actors access them.
- `DIV` reads should be derived from the current internal timer counter state, not from a separately stored visible register byte.
- Any write to `DIV` should invoke the timer's reset semantics regardless of the data value on the bus.
- `TIMA`, `TMA`, and `TAC` should not duplicate timer logic in the bus or CPU; their observable behavior must come from timer-owned state transitions.
- `TAC` writes must be able to trigger the documented one-step TIMA increment glitch when the effective timer signal changes accordingly.

## Shared divider contract with the APU

- The timer should remain the owner of the shared system-counter / divider state from which visible `DIV` is derived.
- The APU frame sequencer should derive its `DIV-APU` tick source from that same divider timeline rather than maintaining a second unrelated free-running divider.
- For the current DMG target, the relevant APU control-clock source is the falling edge of visible-`DIV` bit `4`.
- A write to `DIV` can therefore matter to both subsystems:
  - timer glitch behavior through the effective TIMA signal
  - APU frame-sequencer advancement if the reset produces the documented falling edge seen by `DIV-APU`
- Keep the ownership split explicit: timer owns `DIV` and the shared counter; APU owns `div_apu`, frame-sequencer phase, and the downstream sound clocks.

## Timing / accuracy requirements

- Explain edges, glitches, and event ordering explicitly.
- Do not reduce the model to "increment every X instructions" if finer timing matters.
- Preserve the interaction with interrupt timing and writes to timer registers.
- Express timer behavior on the shared T-cycle timeline of the core.
- The internal timer system counter must advance at `1` step per T-cycle on that shared timeline.
- Keep `DIV`, `TIMA`, and `TAC` coupled through the internal counter and edge logic; do not split them into desynchronized derived counters.
- A write to `DIV` can cause an immediate TIMA increment when it changes the effective timer signal through the relevant falling edge.
- The same `DIV` reset event should remain observable enough for the APU to see whether the `DIV-APU` source edge occurred on that T-cycle.
- A write to `TAC` must reevaluate both the selected counter bit and the enable contribution; TAC writes can therefore trigger the timer glitch behavior and immediate TIMA increment in the relevant cases.
- TIMA overflow must enter an explicit pending/reload sequence before `TMA` is copied and the timer interrupt is requested.
- The shared scheduler should first advance the internal divider/system-counter for the T-cycle, then let the timer derive falling edges and overflow-pipeline transitions from that updated state.
- The timer's delayed `IF` request belongs to the timer-owned overflow pipeline, not to a generic interrupt rule in the scheduler or interrupt controller.
- Writes to `TIMA` and `TMA` near overflow/reload must be modeled against that internal overflow state machine rather than as unconditional register stores.
- When `SkipBoot` synthesizes a post-boot machine state, the timer's hidden `system_counter` and any overflow-related state must be initialized coherently with the visible `DIV`, `TIMA`, `TMA`, and `TAC` snapshot rather than being reset independently.

## Dependencies

- interrupt controller
- T-cycle scheduler or clock source
- bus/MMIO wiring
- model/revision configuration

## Primary references

- Pan Docs timer sections
- AntonioND timing docs
- Gekkio research and Mooneye timer tests

## Open-source emulator references

Priority order:

1. SameBoy
2. binjgb
3. Mooneye GB
4. Danger Boy
5. GameRoy
6. Gambatte

## Tests

- Mooneye timer and DIV/TIMA tests
- DIV read/reset and DIV-write glitch tests
- TAC bit-selection and TAC-write glitch tests
- focused edge-detection and cadence tests for each TAC frequency
- focused write-order and overflow tests
- TIMA overflow-window tests, including reads and writes around pending reload
- delayed timer-request tests that verify `IF.Timer` becomes visible one M-cycle after logical overflow
- separate TIMA-write tests for before overflow, at overflow, during reload, and after reload
- TMA-write timing tests around reload
- separate TMA-write tests for before overflow, just before reload, at reload, and after reload
- timer interrupt integration tests across timer state, `IF`, and CPU-visible servicing timing
- direct-boot continuity tests that verify the first timer-visible ticks after `SkipBoot` remain coherent with the published post-boot `DIV` snapshot

## Implementation notes for this repo

- Keep timer state highly testable.
- Make the source of each timing decision visible in comments or docs.
- Prefer a source-of-truth shape like `system_counter`, `tima`, `tma`, `tac`, `previous_timer_signal`, and an explicit overflow state machine, even if field names differ.
- Expose enough divider-edge information or shared-counter state that the APU can derive `DIV-APU` from the same source instead of cloning timer logic in parallel.
- A pure helper such as `selected_timer_bit(tac)` is a good fit for frequency selection logic.
- `tick()`, `read()`, and `write()` should all be aware of the timer's internal temporal state; register writes are not simple blind setters in the precise model.
- The timer should request its interrupt through the global interrupt controller path, not by mutating unrelated CPU or bus flags ad hoc.
- Treat visible startup values such as `DIV=0xAB` as consequences of a synthesized internal timer state during `SkipBoot`, not as disconnected register literals.

## Recommended implementation order

- implement the internal `system_counter` and derive `DIV` from it
- implement TAC bit selection and the effective timer signal
- implement falling-edge-based TIMA increments
- implement overflow as an explicit temporal state machine
- integrate TIMA/TMA writes with the overflow window
- integrate timer interrupt requests with the global interrupt controller and CPU-visible timing

## Planning note

- Reserve a dedicated work item for TIMA/TMA corner cases during the overflow and reload window; those cases should not be treated as incidental cleanup after the main timer logic.

## Known pitfalls

- treating `DIV` as an independent counter instead of a derived view of the internal counter
- incorrect edge detection
- incrementing TIMA through modular cycle accumulation instead of falling-edge detection
- incorrect reload timing
- implementing reload from `TMA` instantaneously at overflow
- treating `DIV`, `TIMA`, and `TAC` as loosely related registers instead of coupled hardware logic
- mixing interrupt request timing with reload semantics
- setting the visible direct-boot `DIV` register without also choosing a coherent hidden `system_counter`

## Open questions

- which exact overflow state encoding is clearest for the repo while preserving the observable reload window semantics
