# APU

## Scope

Own internal sound hardware state: channels, registers, frame sequencer, DAC state, mixer/master-volume state, HPF state, and the hardware-facing export boundary before host audio backends. Do not own the host audio backend itself.

## Hardware model

Keep channel behavior and frame-sequencer timing explicit. Model the APU as a digital-to-analog pipeline rather than as four channels that directly emit host samples, and keep internal hardware stepping separate from output sampling and playback.

## Responsibilities

- channel state and register semantics
- frame sequencer behavior
- internal sample generation and mixing inputs
- master audio control state such as `NR50`, `NR51`, and `NR52`
- per-channel `dac_enabled` versus `channel_active` state
- stereo mixing, master-volume scaling, and HPF state before host export

## Registers / MMIO

- `NR10`-`NR52`
- wave RAM ownership and access rules

## APU MMIO contract baseline

- The APU should expose register semantics by field, not as a generic byte bank that happens to live in `FF10-FF3F`.
- Registers documented as write-only, such as `NR13`, `NR23`, `NR31`, and `NR41`, should follow an explicit readback policy instead of echoing the last write by default.
- Mixed registers such as `NR14`, `NR24`, `NR34`, `NR44`, and especially `NR52` should keep their read-only and writable fields distinct.
- `NR52` should treat the power bit as writable control state and the channel-on flags as read-only live status.
- Powering the APU off through `NR52` should clear APU register state and make the other APU registers read-only until power is restored, while leaving wave RAM accessibility under the documented hardware policy.
- Trigger bits in the `NRx4` family should perform their channel-start side effects on the write itself.
- Write-only fields such as initial length-timer setup should remain write-only semantically, even if internal channel state later depends on them.

## General APU architecture baseline

- The APU should be modeled as an internal digital-to-analog pipeline, not as four channels that directly emit host samples.
- For the current DMG-family target, that pipeline should remain explicit:
  - per-channel digital generation in the `0..15` range
  - per-channel DAC conversion into analog channel outputs
  - stereo routing and summation under `NR51`
  - per-output master-volume scaling under `NR50`
  - per-output high-pass filtering before export to the host-facing audio layer
- `NR50`, `NR51`, and `NR52` should belong to the master APU path, not to individual channels.
- The master `Apu` owner should contain at least:
  - powered state
  - `NR50`, `NR51`, and `NR52`
  - the frame sequencer / `DIV-APU` counter
  - left/right mixer and HPF state
  - per-channel active status as reflected in `NR52`
- Do not collapse the design into "channel -> final sample" without keeping the digital generator, DAC, mixer, and HPF stages explicit.

## NR52 power-gating baseline

- `NR52` bit `7` should act as real APU power control, comparable in importance to `LCDC.7` for the LCD path.
- Powering the APU off through `NR52` should:
  - clear the other APU registers
  - make those other registers read-only until power is restored
  - leave wave RAM accessible
  - leave the `DIV-APU` counter intact rather than resetting it
- `NR52` bits `0..=3` should remain read-only live status bits that report whether each channel's generation circuit is active.
- The low `NR52` bits must not be treated as DAC-enabled indicators; they report active channels, not DAC state.

## Frame sequencer / DIV-APU baseline

- The project should keep an explicit `div_apu` or equivalent frame-sequencer counter, distinct from the mixer and from per-channel waveform timers.
- For the current DMG target, `div_apu` should advance on the falling edge of `DIV` bit `4` (`1 -> 0`), i.e. at `512` Hz under ordinary operation.
- A write to `DIV` should be able to advance the APU frame-sequencer path immediately if clearing `DIV` produces that same falling edge.
- The frame sequencer must be derived from the same underlying divider/system-counter state already used to expose `DIV`; it must not be modeled as an unrelated free-running audio timer.
- The frame sequencer should provide the slow control clocks only:
  - sound length at `256` Hz
  - CH1 sweep at `128` Hz
  - envelope at `64` Hz
- For future CGB work, keep room for the documented double-speed edge-source change without reworking the ownership model; for the current DMG target, bit `4` is the relevant source.

## Slow control clocks versus fast channel clocks

- The frame sequencer should not clock the base waveform generation of CH1, CH2, CH3, or CH4 directly.
- The frame sequencer should remain limited to:
  - length counters
  - envelope units
  - CH1 sweep
- Each channel should keep its own faster timer path:
  - CH1/CH2 duty-step timing
  - CH3 wave sample timer and sample index
  - CH4 noise/LFSR timer
- The architecture should let those faster per-channel timers arrive later without changing how the shared frame sequencer works.

## DAC state versus channel-active state

- The APU should keep explicit per-channel `dac_enabled` state separate from `channel_active`.
- For CH1, CH2, and CH4, `dac_enabled` should derive from `NRx2 & 0xF8 != 0`.
- For CH3, `dac_enabled` should derive from `NR30` bit `7`.
- Turning a channel DAC off should immediately force that channel inactive.
- A channel may remain inactive while its DAC stays enabled; that should still be representable as an analog zero-output condition rather than as "DAC off".

## Channel-trigger baseline

- Keep a shared trigger concept such as `trigger_channel(channel)` instead of burying all trigger semantics in four unrelated code paths.
- Writing `NRx4` with bit `7` set should execute the channel trigger on that write.
- If the channel DAC is off, the trigger write should not activate the channel.
- Channel deactivation should remain an explicit state transition driven by hardware causes such as:
  - DAC disable
  - length expiry
  - CH1 sweep overflow
- The live channel-active bits exposed through `NR52` must track those activation and deactivation events.

## Mixer, master-volume, and HPF baseline

- `NR51` should route each channel independently into left and/or right output; it is not just a global mute mask.
- `NR50` should scale each output with the documented master-volume semantics where value `0` behaves like factor `1` and value `7` behaves like factor `8`.
- The mixer should sum analog DAC outputs before applying `NR50` master-volume scaling.
- The design should keep room for `VIN`, even if it remains neutral for now.
- Enabling or disabling DACs, changing `NR51`, or changing `NR50` should be allowed to affect output DC offset and therefore the HPF-visible state; those changes are not memoryless.
- The APU output path should include a high-pass filter per output channel, after mixing and master-volume scaling.
- Leaving the HPF out entirely should be treated as a temporary debug mode, not as the target hardware behavior.

## CH1 baseline (pulse + sweep)

- CH1 should be modeled as a composition of explicit sub-blocks rather than as one helper that emits a square wave with a few extra flags attached.
- At minimum, the CH1 state shape should keep explicit fields or equivalent ownership for:
  - `channel_active`
  - `dac_enabled`
  - `period_value`
  - `period_timer`
  - selected duty
  - `duty_step`
  - `length_counter`
  - `length_enabled`
  - envelope timer / pace / direction / current volume
  - `sweep_timer`
  - `sweep_enabled`
  - `sweep_shadow_period`
- CH1 ownership should stay with the channel block itself: the master APU should provide clocks and collect outputs, but it should not own CH1-specific sweep, duty, or envelope internals indirectly through generic helpers.

## CH1 MMIO ownership baseline

- `NR10` through `NR14` should remain owned by the CH1 block rather than by a flat APU register bank.
- `NR10` should be decomposed into sweep pace, sweep direction, and sweep shift/individual-step semantics.
- `NR11` should keep duty (`bits 7-6`) distinct from the write-only initial length field (`bits 5-0`).
- `NR12` should keep initial volume, envelope direction, and envelope pace distinct.
- `NR13` should remain write-only at the MMIO contract layer.
- `NR14` bit `7` should remain the trigger input, while bit `6` should remain the length-enable control with immediate write-time effect.

## CH1 duty and waveform baseline

- CH1 should keep an explicit duty-step counter in the range `0..=7`.
- The waveform should remain an `8`-step pulse waveform selected by `NR11` duty.
- The duty-step counter should not reset when CH1 is retriggered; only powering the APU off should reset the pulse duty-step state.
- Retriggering CH1 should reset the pulse period/frequency timer instead.
- When a pulse channel is first started, the digital output should begin at `0`.
- Keep an explicit follow-up work item for the documented post-power-on quirk where the first CH1/CH2 duty step after the first trigger behaves as if it were step `0`, and duty clocking is disabled until the first trigger.

## CH1 period value and timer baseline

- `NR13` plus the low three bits of `NR14` should form CH1's `11`-bit period value.
- CH1 should keep explicit separation between the period value stored in registers and the in-flight period timer currently timing the sample.
- For the current DMG target, the pulse period timer should be clocked at `1048576` Hz, i.e. once every `4` dots.
- A duty-step advance should occur when the channel's current sample completes, not on every frame-sequencer tick.
- Writes to `NR13` or `NR14` should not change the currently playing sample instantly; the new period should only take effect after the current sample ends.
- Keep a dedicated validation case for CH1 period-write delay rather than burying it inside generic "period changes work" coverage.
- Keep explicit CH1 validation for the internal rule that programmed envelope/sweep pace or period `0` behaves as `8` on the corresponding timer-reload path.

## CH1 DAC and trigger baseline

- CH1 `dac_enabled` should derive from `NR12 & 0xF8 != 0`.
- If the DAC is off, a trigger write to `NR14` must not activate CH1.
- If a write to `NR12` turns the DAC off, CH1 should be disabled immediately.
- `channel_active` and `dac_enabled` must remain distinct CH1 states; an inactive channel with DAC still enabled should still correspond to digital `0` being converted by the DAC.
- CH1 trigger should be represented as one explicit operation that performs the channel's trigger-time state transitions rather than as unrelated side effects scattered across MMIO, envelope, and sweep helpers.
- On CH1 trigger:
  - the channel should become active if the DAC is enabled
  - the period timer should reload from `NR13` / `NR14`
  - the envelope timer should reset
  - current volume should become the initial volume from `NR12`
  - expired length state should be restored to a valid running state
  - sweep should perform its trigger-time initialization
- Keep a dedicated follow-up work item for the documented quirk where triggering CH1/CH2 does not modify the low two bits of the frequency timer.

## CH1 length and envelope baseline

- CH1 should keep an explicit `64`-step length counter.
- That length counter should be clocked only by the frame sequencer's `256` Hz length clock, not by the channel's fast waveform timer.
- `NR14` bit `6` should enable or disable the CH1 length unit immediately on write.
- If the length counter expires while enabled, CH1 should be disabled.
- Extra length clocking on `NR14` writes should remain an explicit CH1 work item; do not treat it as a negligible quirk.
- CH1 should keep envelope timer state and current volume separate from the readable contents of `NR12`.
- The envelope should be clocked from the frame sequencer's `64` Hz envelope clock.
- Envelope pace `0` should disable visible automatic envelope stepping, while still preserving the documented internal timer-reload rule that a programmed pace or period of `0` behaves as `8`.
- Envelope progression must update CH1's internal current volume, not the readable initial-volume bits in `NR12`.
- Reaching volume `0` through the envelope must not disable CH1 by itself.

## CH1 sweep baseline

- CH1 must keep explicit sweep-specific state:
  - `sweep_timer`
  - `sweep_enabled`
  - `sweep_shadow_period`
- Sweep should be clocked from the frame sequencer's `128` Hz CH1 sweep clock.
- On CH1 trigger:
  - the current period should be copied into `sweep_shadow_period`
  - the sweep timer should reset
  - `sweep_enabled` should become true if sweep pace or sweep shift are non-zero, false otherwise
  - if sweep shift is non-zero, sweep calculation and overflow check should run immediately
- Sweep calculation should be represented as an explicit pure calculation over the current shadow period:
  - compute `shadow >> shift`
  - add or subtract depending on sweep direction
  - produce a candidate new period
- If an addition-mode sweep result exceeds `0x7FF`, CH1 should be disabled by sweep overflow.
- Decreasing sweep should not be modeled as a symmetric underflow-based shutdown path; the documented hardware behavior is not symmetric there.
- On a sweep clock while sweep is enabled and pace is non-zero:
  - calculate the new period and perform overflow check
  - if the result is in range and shift is non-zero, write it back to `sweep_shadow_period`, `NR13`, and `NR14`
  - then run a second immediate calculation plus overflow check using the new shadow period, without writing that second result back
- Keep this second overflow check explicit; do not fold it into a generic "next tick will catch it" simplification.
- Writes to `NR13` / `NR14` while sweep is active must not refresh `sweep_shadow_period`; a later sweep tick may therefore overwrite the just-written register value unless CH1 is retriggered.
- Sweep pace `0` should still preserve the documented trigger/overflow semantics and the documented timer-reload rule that a programmed pace or period of `0` behaves as `8`, rather than being simplified to "sweep logic fully off".
- Keep an explicit follow-up work item for the documented CH1 behavior where clearing the sweep direction bit after subtraction-mode calculations can immediately disable the channel.

## CH1 active-state integration baseline

- CH1 should be disabled by exactly these ordinary causes:
  - DAC disable
  - length expiry
  - CH1 sweep overflow
- CH1 should not be disabled merely because the envelope reached volume `0`.
- `NR52` bit `0` should track CH1 activity according to those rules.
- The mixer should consume CH1's resolved current digital output together with its DAC/active state; it should not reconstruct CH1 output by re-reading `NR10` through `NR14`.
- CH1 timing integration should remain split into:
  - fast waveform/period timing on the shared T-cycle timeline
  - slow frame-sequencer clocks for length, envelope, and sweep

## Timing / accuracy requirements

- Keep channel and frame-sequencer timing visible.
- Do not mix backend sampling concerns with hardware state evolution.
- Keep internal APU sequencing compatible with the shared T-cycle timing model, even if audio output is resampled later.
- MMIO-triggered APU events such as `NR52` power transitions and `NRx4` triggers should remain visible on the shared T-cycle timeline.
- Internal APU state should advance from the shared master clock / T-cycle timeline, not from host audio callback cadence or an ad hoc `44.1`/`48` kHz loop.
- The frame-sequencer path should derive its timing from the shared `DIV`/system-counter edge, not from a duplicate software timer hidden inside the audio backend.
- Slow frame-sequencer clocks and fast per-channel waveform timers should remain distinct in the model; do not let the frame sequencer become a surrogate sample clock.
- The core should not depend on emitting one host sample per T-cycle; instead, keep hardware state evolution and host-rate sampling or resampling as separate stages.
- APU power transitions, DAC-enable changes, `NR50` / `NR51` mixer changes, and `NRx4` trigger effects should all remain expressible as ordered T-cycle-visible events.

## Dependencies

- bus/MMIO
- T-cycle scheduler or clock source
- timer / shared divider edge source
- model/revision configuration

## Primary references

- Pan Docs APU sections
- gbdev audio references
- subsystem-specific hardware research where needed

## Open-source emulator references

Priority order:

1. SameBoy
2. Gambatte
3. binjgb
4. GameRoy

## Tests

- audio-focused ROMs where available
- register semantics tests
- frame-sequencer timing tests
- direct-boot register-readback tests for the published post-boot audio snapshot when startup presets bypass firmware execution
- tests for write-only register readback policy
- tests for `NR52` mixed readback and power-gating behavior
- tests that `NRx4` trigger writes cause immediate channel-side effects
- tests that `NR52` power-off clears ordinary audio registers without clearing wave RAM or resetting `DIV-APU`
- tests that `NR52` low bits reflect channel-active state rather than DAC-enabled state
- tests that `DIV-APU` advances on the falling edge of `DIV` bit `4`, including write-to-`DIV` induced extra ticks
- tests that the frame sequencer clocks length, envelope, and CH1 sweep without directly acting as the channels' waveform timer
- tests for `dac_enabled` versus `channel_active`, including DAC-off forcing the channel off and DAC-on plus inactive-channel remaining distinct
- tests for `NR50` / `NR51` routing, master-volume scaling, and the documented "volume 0 still means factor 1" behavior
- tests that HPF-visible state changes when routing, master volume, or DAC state changes produce DC-offset changes
- tests for `NR10`-`NR14` ownership and MMIO semantics, including `NR13` write-only readback policy
- tests for CH1 duty-step behavior, including "retrigger resets timer but not duty step"
- tests for CH1 period-write delay where `NR13` / `NR14` changes apply after the current sample ends
- tests that CH1 trigger reloads period/envelope/sweep state but does not activate the channel while DAC is off
- tests for CH1 length expiry, CH1 envelope progression, and the rule that envelope volume reaching `0` does not disable the channel
- sweep tests covering trigger-time shadow copy, immediate overflow check, timed writeback, second overflow check, and the rule that `NR13` / `NR14` writes do not update the sweep shadow automatically
- dedicated CH1 quirk tests for period-`0`-treated-as-`8`, extra length clocking, low frequency-timer bits on trigger, and the first-duty-step-after-power-on path whenever those behaviors are implemented

## Implementation notes for this repo

- Keep output backend decoupled from the emulation core.
- Favor correctness and clarity before micro-optimizations.
- Visible post-boot `NRxx` register values for `SkipBoot` should come from the centralized boot snapshot rather than ad hoc per-register reset literals spread through APU code.
- Wave RAM accessibility policy should stay explicit and separate from the ordinary `NRxx` register bank contract.
- A shape such as `Apu { powered, div_apu, nr50, nr51, nr52, hpf_left, hpf_right, ch1, ch2, ch3, ch4 }` is a good fit for this repo's ownership model, even if names differ.
- Each channel should expose at least:
  - current digital output
  - `dac_enabled`
  - `channel_active`
  - trigger handling
  - slow control clocks it consumes
  - its own fast timer state
- The mixer should consume already-resolved channel output and DAC state rather than peeking back into raw register storage to reconstruct behavior indirectly.
- Keep a clear API boundary between exact internal audio state and the later host-facing sample or resampler path.
- Reserve explicit follow-up work items for per-channel quirks such as extra length clocking, CH1 sweep details, CH3 retrigger/wave-RAM edge cases, CH4 lock-up, and envelope zombie-mode behavior.
- A channel shape such as `Channel1 { active, dac_enabled, period_value, period_timer, duty, duty_step, length_counter, length_enabled, envelope, sweep }` is a good fit for keeping CH1 readable and testable, even if field names differ.
- Keep CH1 sweep logic isolated enough that trigger-time setup, timed sweep iterations, overflow checks, and shadow-register behavior can each be tested directly.

## Known pitfalls

- mixing host sample rate concerns into hardware timing
- hiding frame-sequencer behavior behind backend callbacks
- treating the APU MMIO range as a plain register array and losing write-only or mixed-field behavior
- confusing `channel_active` with `dac_enabled`
- letting the frame sequencer drive the channels' main waveform timer instead of only their slow control units
- modeling `NR50` / `NR51` as stateless mixer knobs and losing HPF/DC-offset consequences

## Open questions

- what internal sampling interface best preserves determinism and portability
