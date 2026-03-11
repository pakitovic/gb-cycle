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

## CH2 baseline (pulse without sweep)

- CH2 should reuse the same pulse-channel architecture established for CH1 wherever the hardware is actually shared, but without inheriting imaginary sweep state just because the channel is otherwise similar.
- At minimum, the CH2 state shape should keep explicit fields or equivalent ownership for:
  - `channel_active`
  - `dac_enabled`
  - `period_value`
  - `period_timer`
  - selected duty
  - `duty_step`
  - `length_counter`
  - `length_enabled`
  - envelope timer / pace / direction / current volume
- CH2 should explicitly not own sweep-specific state such as a sweep timer, sweep-enabled flag, or shadow-period register.

## CH2 MMIO ownership baseline

- `NR21` through `NR24` should remain owned by the CH2 block rather than by a flat APU register bank.
- `NR21` should keep duty (`bits 7-6`) distinct from the write-only initial length field (`bits 5-0`).
- `NR22` should keep initial volume, envelope direction, and envelope pace distinct.
- `NR23` should remain write-only at the MMIO contract layer.
- `NR24` bit `7` should remain the trigger input, while bit `6` should remain the length-enable control with immediate write-time effect.

## CH2 duty and waveform baseline

- CH2 should keep an explicit duty-step counter in the range `0..=7`.
- The waveform should remain an `8`-step pulse waveform selected by `NR21` duty.
- The duty-step counter should not reset when CH2 is retriggered; only powering the APU off should reset the pulse duty-step state.
- Retriggering CH2 should reset the pulse period/frequency timer instead.
- When a pulse channel is first started, the digital output should begin at `0`.
- Keep an explicit follow-up work item for the documented post-power-on quirk where the first CH1/CH2 duty step after the first trigger behaves as if it were step `0`, and duty clocking is disabled until the first trigger.

## CH2 period value and timer baseline

- `NR23` plus the low three bits of `NR24` should form CH2's `11`-bit period value.
- CH2 should keep explicit separation between the period value stored in registers and the in-flight period timer currently timing the sample.
- For the current DMG target, the pulse period timer should be clocked at `1048576` Hz, i.e. once every `4` dots.
- A duty-step advance should occur when the channel's current sample completes, not on every frame-sequencer tick.
- Writes to `NR23` or `NR24` should not change the currently playing sample instantly; the new period should only take effect after the current sample ends.
- Keep a dedicated validation case for CH2 period-write delay rather than burying it inside generic "period changes work" coverage.

## CH2 DAC and trigger baseline

- CH2 `dac_enabled` should derive from `NR22 & 0xF8 != 0`.
- If the DAC is off, a trigger write to `NR24` must not activate CH2.
- If a write to `NR22` turns the DAC off, CH2 should be disabled immediately.
- `channel_active` and `dac_enabled` must remain distinct CH2 states; an inactive channel with DAC still enabled should still correspond to digital `0` being converted by the DAC.
- CH2 trigger should be represented as one explicit operation that performs the channel's trigger-time state transitions rather than as unrelated side effects scattered across MMIO and envelope helpers.
- On CH2 trigger:
  - the channel should become active if the DAC is enabled
  - the period timer should reload from `NR23` / `NR24`
  - the envelope timer should reset
  - current volume should become the initial volume from `NR22`
  - expired length state should be restored to a valid running state
- Keep a dedicated follow-up work item for the documented quirk where triggering CH1/CH2 does not modify the low two bits of the frequency timer.

## CH2 length and envelope baseline

- CH2 should keep an explicit `64`-step length counter.
- That length counter should be clocked only by the frame sequencer's `256` Hz length clock, not by the channel's fast waveform timer.
- `NR24` bit `6` should enable or disable the CH2 length unit immediately on write.
- If the length counter expires while enabled, CH2 should be disabled.
- Extra length clocking on `NR24` writes should remain an explicit CH2 work item and should reuse the same general infrastructure as CH1 rather than a parallel incompatible implementation.
- CH2 should keep envelope timer state and current volume separate from the readable contents of `NR22`.
- The envelope should be clocked from the frame sequencer's `64` Hz envelope clock.
- Envelope pace `0` should disable visible automatic envelope stepping, while still preserving the documented internal timer-reload rule that a programmed pace or period of `0` behaves as `8`.
- Envelope progression must update CH2's internal current volume, not the readable initial-volume bits in `NR22`.
- Reaching volume `0` through the envelope must not disable CH2 by itself.

## CH2 active-state integration and shared quirks baseline

- CH2 should be disabled by exactly these ordinary causes:
  - DAC disable
  - length expiry
- CH2 should not be disabled merely because the envelope reached volume `0`.
- `NR52` bit `1` should track CH2 activity according to those rules.
- CH2 should explicitly reserve the pulse-channel quirks it shares with CH1:
  - programmed envelope pace or period `0` behaving as `8` on the timer-reload path
  - the first-duty-step-after-power-on behavior
  - low frequency-timer bits preserved on trigger
  - extra length clocking on `NR24` writes
- These quirks should live in CH2 trigger/timer state rather than in post-mix audio patches.
- The mixer should consume CH2's resolved current digital output together with its DAC/active state; it should not reconstruct CH2 output by re-reading `NR21` through `NR24`.
- CH2 should expose distinct temporal inputs for:
  - fast pulse timing on the shared T-cycle timeline
  - slow frame-sequencer clocks for length and envelope
- CH2's resolved digital output should be a function of its internal active state, DAC state, duty position, and current envelope-derived volume rather than a fresh interpretation of MMIO register contents at mix time.
- CH2 timing integration should remain split into:
  - fast waveform/period timing on the shared T-cycle timeline
  - slow frame-sequencer clocks for length and envelope

## CH3 baseline (wave channel)

- CH3 should be modeled as a distinct wave-channel block rather than as a pulse channel with a replaceable waveform.
- At minimum, the CH3 state shape should keep explicit fields or equivalent ownership for:
  - `channel_active`
  - `dac_enabled`
  - wave RAM storage
  - `sample_index`
  - `sample_buffer`
  - selected output level
  - `period_value`
  - `period_timer`
  - `length_counter`
  - `length_enabled`
- CH3 should explicitly not inherit pulse-only machinery such as duty-step state, envelope state, or CH1 sweep state.

## CH3 MMIO ownership baseline

- `NR30` through `NR34` should remain owned by the CH3 block rather than by a flat APU register bank.
- `NR30` bit `7` should remain the CH3 DAC-enable control.
- `NR31` should remain write-only at the MMIO contract layer and should represent the initial length write path rather than a readable live counter.
- `NR32` should be treated as CH3's digital output-level control, not as an analog mixer-volume knob.
- `NR33` should remain write-only at the MMIO contract layer.
- `NR34` bit `7` should remain the trigger input, while bit `6` should remain the length-enable control with immediate write-time effect.

## CH3 wave RAM baseline

- CH3 should own an explicit wave RAM of `16` bytes, exposed through the ordinary wave-RAM MMIO path rather than hidden behind abstract sample storage.
- That wave RAM should represent `32` logical `4`-bit samples, packed as two nibbles per byte.
- CH3 sample fetch should consume concrete nibbles from that wave RAM according to the current internal sample index rather than from a pre-expanded abstract wavetable detached from MMIO-visible bytes.
- Wave RAM should remain accessible under the documented hardware policy when the APU is powered off through `NR52`; it must not be cleared just because ordinary audio registers reset.

## CH3 sample index and sample-buffer baseline

- CH3 should keep an explicit `sample_index` in the range `0..=31`.
- CH3 should keep an explicit `sample_buffer` separate from wave RAM storage.
- Each sample-index advance should read the corresponding nibble from wave RAM and load that nibble into the sample buffer.
- CH3 digital output should come from the buffered sample value, not from a fresh live wave-RAM read at mix time.
- Retriggering CH3 should not automatically clear or refill the sample buffer; the channel should continue outputting the last buffered sample until the next wave-RAM fetch occurs.

## CH3 startup and first-sample baseline

- After APU power-on, CH3 should begin with its sample buffer at digital `0`.
- CH3 startup should explicitly reserve the documented first-sample quirk where sample `0` is skipped when first starting the channel and the first post-trigger output is not a naive immediate replay of wave-table sample `0`.
- A CH3 retrigger should therefore preserve the previously buffered sample until the next internal wave-RAM read occurs rather than forcing an immediate load of wave-table sample `0` or clearing the buffer automatically.
- Keep this startup quirk as explicit CH3 trigger/sample-fetch follow-up work rather than flattening it into a generic "wave channel begins at sample 0" simplification.

## CH3 period value and timer baseline

- `NR33` plus the low three bits of `NR34` should form CH3's `11`-bit period value.
- CH3 should keep explicit separation between the period value stored in registers and the in-flight period timer currently timing the next sample fetch.
- For the current DMG target, the CH3 period timer should be clocked at `2097152` Hz, i.e. once every `2` dots.
- CH3 should advance its `sample_index` at the channel's sample rate, with the `32`-sample waveform reached by successive wave-RAM fetches rather than pulse-duty stepping.
- Writes to `NR33` or `NR34` should not change the currently buffered output sample instantly; the new period should take effect only after the next wave-RAM read boundary.
- Keep a dedicated validation case for CH3 period-write delay rather than burying it inside generic "period changes work" coverage.

## CH3 output-level baseline

- CH3 should not own an envelope unit.
- `NR32` should act as digital attenuation on the buffered sample value before DAC conversion rather than as analog volume control in the mixer.
- `NR32 = 00` should mute CH3 digitally.
- `NR32 = 01` should output the buffered sample unshifted.
- `NR32 = 10` should output the buffered sample shifted right by `1`.
- `NR32 = 11` should output the buffered sample shifted right by `2`.
- Mid-playback writes to `NR32` should remain visible in CH3's resolved digital output immediately enough that mixer/DC-offset/HPF-visible behavior can observe the change.

## CH3 DAC and trigger baseline

- CH3 `dac_enabled` should derive exclusively from `NR30` bit `7`.
- If the DAC is off, a trigger write to `NR34` must not activate CH3.
- If a write to `NR30` clears bit `7`, CH3 should be disabled immediately.
- `channel_active` and `dac_enabled` must remain distinct CH3 states; a DAC-enabled but inactive CH3 should still correspond to digital `0` rather than to "channel off equals DAC off".
- CH3 trigger should be represented as one explicit operation that performs the channel's trigger-time state transitions rather than as unrelated side effects scattered across MMIO and wave-RAM helpers.
- On CH3 trigger:
  - the channel should become active if the DAC is enabled
  - the period timer should reload from `NR33` / `NR34`
  - the sample index should reset to the trigger-defined starting position
  - the effective output level should come from the current `NR32` setting
  - expired length state should be restored to a valid running state
- CH3 trigger should not refill the sample buffer automatically from wave RAM.

## CH3 length baseline

- CH3 should keep an explicit `256`-step length counter.
- That length counter should be clocked only by the frame sequencer's `256` Hz length clock, not by the channel's fast sample timer.
- `NR34` bit `6` should enable or disable the CH3 length unit immediately on write.
- If the length counter expires while enabled, CH3 should be disabled.
- Extra length clocking on `NR34` writes should remain an explicit CH3 work item rather than disappearing behind generic channel code.
- Keep an explicit CH3 follow-up item for the documented trigger-with-length-0 quirk where the effective reloaded length can become `255` instead of `256` depending on frame-sequencer state.

## CH3 wave RAM access and DMG retrigger-corruption baseline

- CH3 wave-RAM access while the channel is active should remain under an explicit hardware policy rather than being treated as always-free RAM with no side effects.
- Keep a dedicated CH3 work item for the exact wave-RAM access policy while CH3 is active, even if the first implementation leaves some fine details isolated.
- Keep a dedicated CH3 DMG-family work item for wave-RAM corruption caused by retriggering CH3 at the exact time of an internal wave-RAM read.
- That corruption path should stay model-gated to DMG-family behavior rather than leaking automatically into future unaffected models.
- The corruption decision should depend on the exact internal byte-read position when the retrigger occurs, not on a vague "channel was active" condition.
- The corruption model should distinguish reads in bytes `0..=3` from reads in bytes `4..=15`.
- For reads in bytes `0..=3`, the documented special case should overwrite only the first byte of wave RAM with the byte currently being read rather than applying the later aligned-block copy rule.
- For reads in bytes `4..=15`, the first four bytes of wave RAM should be overwritten from the aligned `4`-byte block documented for the internal source position rather than by an undifferentiated "some data got corrupted" shortcut.

## CH3 active-state integration and timing baseline

- CH3 should be disabled by exactly these ordinary causes:
  - DAC disable
  - length expiry
- CH3 should not invent a shutdown path merely because the buffered sample is `0`, because `NR32` is mute, or because the waveform content looks silent.
- `NR52` bit `2` should track CH3 activity according to those rules.
- The mixer should consume CH3's resolved current digital output together with its DAC/active state; it should not reconstruct CH3 output by re-reading `NR30` through `NR34`.
- CH3 should expose distinct temporal inputs for:
  - fast sample timing and wave-RAM fetch progression on the shared T-cycle timeline
  - slow frame-sequencer clocks for length only
- CH3's resolved digital output should be a function of its internal active state, DAC state, buffered sample, and current `NR32` output level rather than of raw MMIO register rereads at mix time.

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
- direct-boot continuity tests that verify the first APU-visible ticks after `SkipBoot` remain coherent with the published post-boot audio snapshot instead of restarting `DIV-APU`, frame-sequencer, DAC, or HPF-visible state from an unrelated zeroed phase
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
- tests for `NR21`-`NR24` ownership and MMIO semantics, including `NR23` write-only readback policy
- tests for CH2 duty-step behavior, including "retrigger resets timer but not duty step"
- tests for CH2 period-write delay where `NR23` / `NR24` changes apply after the current sample ends
- tests that CH2 trigger reloads period/envelope state but does not activate the channel while DAC is off
- tests for CH2 length expiry, CH2 envelope progression, and the rule that envelope volume reaching `0` does not disable the channel
- dedicated CH2 quirk tests for envelope-timer `0 -> 8`, extra length clocking, low frequency-timer bits on trigger, and the first-duty-step-after-power-on path whenever those behaviors are implemented
- tests for `NR30`-`NR34` ownership and MMIO semantics, including `NR31` / `NR33` write-only readback policy
- tests that wave RAM remains accessible and is not cleared by `NR52` power-off
- tests for CH3 period-timer cadence at one tick every `2` dots, `32`-sample index progression, and sample-buffer reload from wave RAM
- tests for CH3 period-write delay where `NR33` / `NR34` changes apply only after the next wave-RAM read boundary
- tests that CH3 trigger reloads timer/index state but does not activate the channel while DAC is off and does not clear or refill the sample buffer automatically
- tests for CH3 length expiry, `NR32` digital output-level semantics, and the rule that `NR32` mute is not equivalent to DAC-off or channel-off
- dedicated CH3 quirk tests for digital-`0` startup state, skipped-first-sample / first-buffer behavior, explicit wave-RAM access policy while active, trigger-with-length-0 behavior, and DMG-family retrigger corruption keyed both to the exact byte-read position and to the aligned source block whenever those behaviors are implemented

## Implementation notes for this repo

- Keep output backend decoupled from the emulation core.
- Favor correctness and clarity before micro-optimizations.
- Visible post-boot `NRxx` register values for `SkipBoot` should come from the centralized boot snapshot rather than ad hoc per-register reset literals spread through APU code.
- `SkipBoot` should also synthesize coherent hidden APU timing state such as `DIV-APU`, frame-sequencer phase, channel-active/DAC state, and HPF-visible history rather than pairing the published post-boot `NRxx` values with a contradictory zeroed internal audio phase.
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
- A sibling shape such as `Channel2 { active, dac_enabled, period_value, period_timer, duty, duty_step, length_counter, length_enabled, envelope }` is a good fit for reusing the pulse-channel base without carrying sweep-only state into CH2.
- A distinct shape such as `Channel3 { active, dac_enabled, wave_ram, sample_index, sample_buffer, output_level, period_value, period_timer, length_counter, length_enabled }` is a good fit for keeping CH3 separate from pulse-channel assumptions and making wave-RAM fetch behavior directly testable.

## Known pitfalls

- mixing host sample rate concerns into hardware timing
- hiding frame-sequencer behavior behind backend callbacks
- treating the APU MMIO range as a plain register array and losing write-only or mixed-field behavior
- confusing `channel_active` with `dac_enabled`
- letting the frame sequencer drive the channels' main waveform timer instead of only their slow control units
- modeling `NR50` / `NR51` as stateless mixer knobs and losing HPF/DC-offset consequences
- treating CH3 as a pulse channel with a custom waveform and thereby losing wave RAM, buffered-sample, output-level, and retrigger-specific behavior

## Open questions

- what internal sampling interface best preserves determinism and portability
