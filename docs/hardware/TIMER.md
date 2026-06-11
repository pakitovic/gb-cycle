# TIMER

## Scope

Own `DIV`, `TIMA`, `TMA`, `TAC`, their internal timing state, overflow behavior, and interrupt request generation.

## Hardware model

Model the timer as edge-sensitive hardware, not as a periodic software counter incremented every few instructions. The source of truth should be an internal `16`-bit system counter advanced by the shared master clock, with `DIV` and TIMA-driving events derived from that counter rather than maintained as unrelated software counters.

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
- In the current DMG-family baseline in this repo, executing `STOP` also resets that same shared divider/system-counter state. If `STOP` remains active, `DIV` stays at `0x00` until wake; if `STOP` is cancelled on entry, the counter resumes immediately from zero.
- TIMA increments should come from a falling-edge (`1 -> 0`) detection on the effective timer signal, not from a generic "every N cycles" accumulator.
- The effective timer signal on DMG is `timer_enable && selected_counter_bit`.
- The TAC frequency selection should be modeled as internal counter-bit selection, using the DMG mapping:
  - `00 -> bit 9`
  - `01 -> bit 3`
  - `10 -> bit 5`
  - `11 -> bit 7`
- Timer overflow should be modeled as a temporal process with explicit pending/reload state; do not collapse overflow, reload from `TMA`, and interrupt request into one instant write-like event.
- On DMG, the timer interrupt request does not become visible at the same logical moment as overflow detection. The `TMA` reload and timer request into `IF` arrive `4` T-cycles later (`1` M-cycle).
- In the current Phase `2.7` baseline for this repo, the implemented closed path includes falling-edge TIMA increments, `DIV` / `TAC` glitch-triggered increments, `TIMA` writes that cancel a pending reload before it commits, `TMA` writes that affect a later pending reload, and timer-request visibility through the shared scheduler after the `4` T-cycle delay.

## MMIO contract baseline

- `DIV`, `TIMA`, `TMA`, and `TAC` belong to the timer subsystem; MMIO is only the external contract by which other actors access them.
- `DIV` reads should be derived from the current internal timer counter state, not from a separately stored visible register byte.
- Any write to `DIV` should invoke the timer's reset semantics regardless of the data value on the bus.
- For the current DMG-family baseline, `TAC` bits `7..=3` should read back as `1`, while bits `2..=0` reflect the live timer enable/select state.
- `TIMA`, `TMA`, and `TAC` should not duplicate timer logic in the bus or CPU; their observable behavior must come from timer-owned state transitions.
- `TAC` writes must be able to trigger the documented one-step TIMA increment glitch when the effective timer signal changes accordingly.

## Shared divider contract with the APU

- The timer should remain the owner of the shared system-counter / divider state from which visible `DIV` is derived.
- The APU frame sequencer should derive its `DIV-APU` tick source from that same divider timeline rather than maintaining a second unrelated free-running divider.
- For the current DMG target, the relevant APU control-clock source is the falling edge of visible-`DIV` bit `4`.
- A write to `DIV` can therefore matter to both subsystems:
  - timer glitch behavior through the effective TIMA signal
  - APU frame-sequencer advancement if the reset produces the documented falling edge seen by `DIV-APU`
- CPU-driven `DIV` MMIO writes currently seed a narrow `2` T-cycle offset used only by the APU frame-sequencer source signal after the reset; this does not change the visible `DIV` register, the literal `system_counter` snapshot, or TIMA's selected-counter-bit path.
- `STOP` divider resets reuse the timer-owned edge reporting and TIMA glitch path, but they must not inherit that CPU-`DIV`-write-only APU frame-sequencer offset unless a stronger STOP-specific hardware oracle requires it.
- Keep the ownership split explicit: timer owns `DIV` and the shared counter; APU owns `div_apu`, frame-sequencer phase, and the downstream sound clocks.
- The timer-owned divider path should expose enough explicit edge information that autonomous ticks can publish `DIV`-derived events to the scheduler and `DIV` reset writes can synchronously report whether an immediate `DIV-APU` edge occurred on that write.

## CGB speed-domain baseline

- Phase 10 Slice 2 keeps the timer-owned `system_counter` as the source of truth across speed changes instead of introducing a second double-speed accumulator.
- In the current CPU-visible timing baseline, both normal speed and native CGB double speed advance the timer-owned internal counter by `1` per CPU-visible scheduler T-cycle. This preserves the `DIV` cadence observed by Daid `speed_switch_timing_div.gbc`; later scheduler work may refine CPU-vs-LCD wall-clock domains without changing the CPU-visible `DIV` read sequence.
- `DIV` remains a derived view of the internal counter, and TIMA still increments from falling-edge detection on `timer_enable && selected_counter_bit`; speed mode changes domain selection for downstream consumers, not the edge rule.
- `DIV` writes and `STOP` divider resets use the same speed-aware edge path as autonomous ticks, including immediate TIMA glitch effects and the synchronous report of whether an APU frame-sequencer edge occurred; only CPU `DIV` writes apply the repo-local APU frame-sequencer offset described above.
- To keep the APU frame sequencer in its documented undoubled timing domain relative to the CGB speed switch, the shared `DIV-APU` source is counter bit `12` in normal speed and counter bit `13` in double speed.
- The serial controller consumes the same speed-domain contract for its baseline internal-clock edge selection, but full CGB serial high-speed `SC.1` behavior belongs to the later serial-owned CGB slice.
- LCD/PPU LY/STAT timing must not derive from this counter cadence directly; CGB speed state is a published scheduler-domain input, not a generic multiplier for every subsystem.

## Timing / accuracy requirements

- Explain edges, glitches, and event ordering explicitly.
- Do not reduce the model to "increment every X instructions" if finer timing matters.
- Preserve the interaction with interrupt timing and writes to timer registers.
- Express timer behavior on the shared T-cycle timeline of the core.
- The internal timer system counter must advance through the shared CPU-visible speed-domain contract: `1` step per scheduler T-cycle in both normal speed and native CGB double speed, with downstream consumers selecting the appropriate divider bit or cadence instead of mutating visible `DIV` progression.
- Keep `DIV`, `TIMA`, and `TAC` coupled through the internal counter and edge logic; do not split them into desynchronized derived counters.
- A write to `DIV` can cause an immediate TIMA increment when it changes the effective timer signal through the relevant falling edge.
- The same `DIV` reset event should remain observable enough for the APU to see whether the `DIV-APU` source edge occurred on that T-cycle, while keeping the CPU-`DIV`-write offset separate from STOP resets.
- A write to `TAC` must reevaluate both the selected counter bit and the enable contribution; TAC writes can therefore trigger the timer glitch behavior and immediate TIMA increment in the relevant cases.
- If a `DIV` or `TAC` write-triggered glitch is itself the event that overflows `TIMA`, the reload / IRQ window still has to stay aligned to the shared T-cycle timeline instead of silently slipping by one extra cycle just because the timer's autonomous tick for that same cycle already ran earlier in the scheduler.
- TIMA overflow must enter an explicit pending/reload sequence before `TMA` is copied and the timer interrupt is requested.
- The shared scheduler should first advance the internal divider/system-counter for the T-cycle, then let the timer derive falling edges and overflow-pipeline transitions from that updated state.
- The timer's delayed `IF` request belongs to the timer-owned overflow pipeline, not to a generic interrupt rule in the scheduler or interrupt controller.
- Once the timer does request `IF`, CPU MMIO reads of `FF0F` in that same shared T-cycle should be able to observe the newly raised timer bit even if the repo's explicit interrupt-aggregation checkpoint and interrupt-accept decision still happen later in the cycle.
- Writes to `TIMA` and `TMA` near overflow/reload must be modeled against that internal overflow state machine rather than as unconditional register stores.
- Exact same-cycle `TIMA` / `TMA` write arbitration on the reload T-cycle itself should remain explicit work; do not silently claim that the current baseline already closes every reload-window corner case just because pre-reload writes and delayed `IF` timing are covered.
- When `SkipBoot` synthesizes a post-boot machine state, the timer's hidden `system_counter` and any overflow-related state must be initialized coherently with the visible `DIV`, `TIMA`, `TMA`, and `TAC` snapshot rather than being reset independently. The current direct-boot seeds are `0xABC8` for the DMG-family continuity profile, `0x2880` for explicit CGB0 boot validation, `0x2674` for missing CGB-family cartridges and DMG-compatible CGB headers validated by Mooneye `misc/boot_div-cgbABCDE.gb`, `0x2678` for AGB-family DMG-compatible headers validated by Mooneye `misc/boot_div-A.gb` against `cgb_agb_boot.bin`, `0x1E84` for the native CGB non-Nintendo old-licensee bucket used by Ashiepaws `bully.gb`, and `0x1E98` for native CGB old-licensee `$33` headers with binary-zero new-licensee bytes used by Nitro2k01 `whichboot.gb`. `CpuAgb0` currently shares the `CpuAgbA` AGB-family direct-start timer buckets because the promoted AGB0 evidence identifies a distinct RealBoot asset, not a distinct synthetic timer handoff state.

## Dependencies

- interrupt controller
- T-cycle scheduler or clock source
- bus/MMIO wiring
- model/revision configuration

## Primary references

- Pan Docs timer sections
- AntonioND timing docs
- Gekkio research and Mooneye timer tests

## Tests

- Mooneye timer and DIV/TIMA tests
- DIV read/reset and DIV-write glitch tests
- TAC bit-selection and TAC-write glitch tests
- TAC readback tests that keep bits `7..=3` forced high while bits `2..=0` reflect live timer control state
- focused edge-detection and cadence tests for each TAC frequency
- focused write-order and overflow tests
- TIMA overflow-window tests, including reads and writes around pending reload
- delayed timer-request tests that verify `IF.Timer` becomes visible `4` T-cycles (`1` M-cycle) after logical overflow
- CGB speed-domain tests that verify CPU-visible double-speed divider cadence, unchanged falling-edge semantics, speed-aware `DIV` write effects, `STOP` switch reset behavior, and the undoubled APU frame-sequencer edge domain
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
- For the current DMG `SkipBoot` baseline, the APU-side `div_apu` seed should be derived from the same full hidden `system_counter` phase as the timer, including the current low-byte `0xC8` boot seed, rather than from visible `DIV` alone.
- A pure helper such as `selected_timer_bit(tac)` is a good fit for frequency selection logic.
- `tick()`, `read()`, and `write()` should all be aware of the timer's internal temporal state; register writes are not simple blind setters in the precise model.
- The timer should request its interrupt through the global interrupt controller path, not by mutating unrelated CPU or bus flags ad hoc.
- Treat visible startup values such as `DIV=0xAB` as consequences of a synthesized internal timer state during `SkipBoot`, not as disconnected register literals.
- For the current DMG / MGB `SkipBoot` baseline, the synthesized timer state should seed the internal system counter to `0xABC8`, not merely `0xAB00`, so the first post-boot `DIV` edges match Mooneye's DMG-family `boot_div` timing. The matching later-DMG / MGB `RealBoot` path seeds the timer with a `0x0024` power-on reset offset so executing the real boot ROM reaches the same `0xABC8` handoff phase instead of starting the divider from zero. For CGB-family direct start, the synthesized timer state is selected from the cartridge header and revision: explicit CGB0 uses `0x2880`, missing CGB-family cartridges and DMG-compatible CGB headers seed `0x2674`, not merely `0x2600`, so Mooneye `misc/boot_div-cgbABCDE.gb` remains the ABCDE baseline; AGB-family DMG-compatible headers seed `0x2678` so Mooneye `misc/boot_div-A.gb` matches the verified `cgb_agb_boot.bin` handoff phase; the validated native CGB non-Nintendo old-licensee bucket seeds `0x1E84` to match gb-cycle's explicit standard `cgb_boot.bin` handoff observation for Ashiepaws `bully.gb`; and the native CGB old-licensee `$33` plus binary-zero new-licensee bucket seeds `0x1E98` to match Nitro2k01 `whichboot.gb` without manifest-level startup timer overrides. `CpuAgb0` does not get a distinct direct-start timer seed until hardware evidence shows one.

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
