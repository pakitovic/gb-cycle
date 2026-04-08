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
- The internal state shape should stay coherent as one APU-owned structure with explicit master-control ownership plus per-channel register and runtime state, rather than a flat byte bank plus synthetic status masks.
- Do not collapse the design into "channel -> final sample" without keeping the digital generator, DAC, mixer, and HPF stages explicit.

## NR52 power-gating baseline

- `NR52` bit `7` should act as real APU power control, comparable in importance to `LCDC.7` for the LCD path.
- Powering the APU off through `NR52` should:
  - clear the other APU registers
  - make those other registers read-only until power is restored
  - leave wave RAM accessible
  - leave the `DIV-APU` counter intact rather than resetting it
- Powering the APU on through `NR52` should reset the frame sequencer so the
  next step is `0`, while still deriving the timing of that next step from the
  live shared divider source rather than from a synthetic fresh timer.
- If the shared `DIV-APU` source bit is already high when `NR52` powers the APU
  on, the first frame-sequencer event should be skipped and the next real step-0
  edge should therefore arrive `8192` T-cycles later than the low-source case.
- Applying direct-boot or startup audio state with `powered = false` should converge to that same observable powered-off contract, except for the configured wave-RAM startup contents and the preserved `DIV-APU` phase.
- `NR52` bits `0..=3` should remain read-only live status bits that report whether each channel's generation circuit is active.
- The low `NR52` bits must not be treated as DAC-enabled indicators; they report active channels, not DAC state.

## Frame sequencer / DIV-APU baseline

- The project should keep an explicit `div_apu` or equivalent frame-sequencer counter, distinct from the mixer and from per-channel waveform timers.
- For the current DMG target, `div_apu` should advance on the falling edge of `DIV` bit `4` (`1 -> 0`), i.e. at `512` Hz under ordinary operation.
- A write to `DIV` should be able to advance the APU frame-sequencer path immediately if clearing `DIV` produces that same falling edge.
- The frame sequencer must be derived from the same underlying divider/system-counter state already used to expose `DIV`; it must not be modeled as an unrelated free-running audio timer.
- `div_apu` startup / direct-boot phase should therefore be synthesized from the same hidden shared-counter phase that produces visible `DIV`, not from an unrelated zeroed audio seed or from the visible `DIV` byte alone.
- The frame sequencer should provide the slow control clocks only:
  - sound length at `256` Hz
  - CH1 sweep at `128` Hz
  - envelope at `64` Hz
- For future CGB work, keep room for the documented double-speed edge-source change without reworking the ownership model; for the current DMG target, bit `4` is the relevant source.

## Scheduler integration baseline

- The shared scheduler should advance the shared divider/system-counter and resolve `DIV`-derived edge events before ticking the APU for that T-cycle.
- The APU should then consume those already-derived slow-control events together with its own fast per-channel timers on the same T-cycle timeline.
- `DIV` reset MMIO writes should remain able to notify the APU immediately when the reset itself produces the `DIV-APU` falling edge; that write-time path should reuse timer-owned divider knowledge rather than cloning divider logic inside the APU or bus.
- The scheduler must not rederive frame-sequencer rules internally; it only provides ordered clock inputs and calls into the APU at the correct phase.

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

## Final output-pipeline baseline

- The final APU output path should remain explicit as:
  - resolved per-channel digital output
  - per-channel DAC conversion
  - stereo routing and summation under `NR51`
  - per-output master-volume scaling under `NR50`
  - per-output high-pass filtering
  - host-facing capture / resampler boundary
- Do not collapse the design into "sum four channel states and normalize to host PCM" without the DAC, mixer, and HPF stages remaining explicit.
- Final mixing should remain owned by the master APU path rather than being split between channel helpers and the host audio backend.
- Before `NR50` scaling and HPF, each stereo analog bus should be allowed to represent the true mixed hardware-domain range produced by summing up to four channel DAC outputs; do not pre-normalize the core's internal analog signal just because the host sink later wants `float` or `int16`.

## DAC conversion baseline

- Each channel should expose a resolved digital output in the hardware `0..15` domain before DAC conversion rather than a pre-mixed host-ready sample.
- The APU should keep an explicit DAC stage per channel.
- When a channel DAC is enabled, digital values `0..15` should map linearly into the analog `-1..1` range with the documented negative slope:
  - digital `0 -> analog +1`
  - digital `15 -> analog -1`
- An inactive channel with its DAC still enabled should therefore contribute the analog level corresponding to digital `0`; "inactive" must stay distinct from "DAC off".
- The DAC-off path should remain explicit rather than being faked as one more ordinary `0..15` digital conversion case.
- For the current DMG-family target, leave room for the documented model-dependent transition toward analog `0` when a DAC turns off instead of baking that behavior into ordinary digital-code conversion.
- `NR52` low bits should continue to report channel-active state rather than DAC-enabled state.

## NR51 stereo-routing baseline

- `NR51` should be modeled as a true routing matrix from each channel DAC output into the left and right analog buses, not as a musical mute or volume control.
- The left mixer input should sum only the channel DAC outputs whose left-routing bits are enabled in `NR51`.
- The right mixer input should sum only the channel DAC outputs whose right-routing bits are enabled in `NR51`.
- Writes to `NR51` should affect routing immediately on the shared T-cycle timeline.
- `NR51` changes should occur before the HPF stage, because they change the DC offset seen by the filter and can therefore produce the documented pops.

## NR50 master-volume baseline

- `NR50` should scale each stereo output after analog summation and before the HPF stage.
- `NR50` output volume level `0` should not mute a non-silent signal; it should remain the documented minimum non-zero factor, while level `7` should remain the documented maximum factor.
- `NR50` should not be treated as a backend-oriented "final gain" knob that is free to renormalize the core's analog range arbitrarily.
- Writes to `NR50` should affect output immediately on the shared T-cycle timeline.
- `NR50` changes should remain part of the same DC-offset / pop-sensitive hardware path as routing and DAC-enable changes rather than being deferred to host-buffer boundaries.
- Keep an explicit slot for `VIN` in the master mixer path even if the current DMG target leaves it neutral.

## HPF, DC-offset, and pops baseline

- The APU should keep one independent high-pass filter state per stereo output.
- The HPF should be placed after stereo routing and `NR50` master-volume scaling, not before them.
- The HPF must keep persistent state across captured samples; it is not a memoryless per-sample transform.
- Leaving the HPF out may exist as a temporary debug mode, but not as the target DMG-family behavior.
- The design should leave room for later model-specific HPF aggressiveness differences instead of hard-wiring one backend-oriented filter constant forever.
- Documented pops caused by DAC enable changes, `NR51` routing changes, or `NR50` volume changes should emerge from the modeled DC-offset step plus HPF response rather than from ad hoc smoothing or suppression in the host backend.
- A debug tool may visualize DC offset or pop-inducing events, but the default emulation path should not erase those hardware-visible artifacts.

## Channel-output ownership baseline

- Each channel should own the logic that resolves its current digital output before DAC conversion:
  - CH1 / CH2 after duty and current envelope-derived volume
  - CH3 after wave-sample buffering and `NR32` output-level attenuation
  - CH4 as digital `0` or current envelope-derived volume according to the LFSR output bit
- The master APU path should own the digital-to-analog conversion, stereo routing, master-volume scaling, and HPF stages.
- The stereo mixer should not need to know channel-internal concepts such as duty step, sample index, or LFSR state; it should consume resolved DAC inputs from the channels rather than re-deriving channel behavior from MMIO or internal waveform state.

## APU power-off and mixer-state baseline

- Powering the APU off through `NR52` should stop active channel contributions from reaching the live stereo mix.
- That power-off path should not clear wave RAM or reset `DIV-APU`, but it should clear channel-active state, DAC-visible routing participation, and ordinary audio-register state coherently enough that the master mix no longer behaves as if stale channels were still alive.
- Powering the APU back on should restart from a coherent mixer state rather than from partially preserved per-channel contributions.
- After power restoration, later software-visible DAC, routing, and volume changes should still be able to produce the documented pop behavior through the ordinary modeled output path.

## Host-facing output-boundary baseline

- Internal APU state should continue to advance on the shared T-cycle timeline of the emulator rather than on the host audio callback cadence.
- Keep one explicit boundary between:
  - the T-cycle-accurate APU core producing analog stereo state
  - the later host-facing sample capture / resampler / export path
- The host sample rate, host buffer size, or host callback timing must not feed back into channel timers, mixer semantics, HPF behavior, or pop generation.
- The host-facing resampler should only change representation and cadence, not hardware semantics.
- The core APU should therefore remain runnable without a real host audio backend, exposing deterministic internal analog output or captured samples for tests and offline validation.

## Sample-capture and normalization baseline

- The project should keep an explicit sample-capture policy that snapshots the internal post-HPF stereo analog output into a host-facing stream.
- That sample-capture policy should remain independent from the T-cycle scheduler logic that advances the hardware itself.
- Changing host output rate, such as `44.1` kHz versus `48` kHz, should not require changing the internal APU hardware model.
- The design should leave room for replacing the host-facing resampler later without rewriting the DAC, mixer, or HPF logic.
- Conversion from the core's internal analog representation into host `float` or `int16` output should be a final representation step after HPF, not part of the hardware model itself.
- The core should keep a sufficiently precise internal analog representation so host-format conversion does not force the hardware model to clip or renormalize early.

Current branch baseline, March 31, 2026:

- The master APU path now exposes an explicit output snapshot in the core branch baseline: resolved per-channel digital outputs, per-channel DAC outputs, stereo mixer output after `NR51`, stereo master output after `NR50`, plus post-HPF output and persistent HPF capacitor state.
- The current DMG-facing DAC stage is now explicit in the core branch baseline instead of remaining an inferred later backend concern: enabled-DAC digital `0..15` is mapped linearly onto a negative-slope internal analog range, while DAC-off remains a separate explicit zero-output path rather than being collapsed into one more ordinary converted sample.
- An inactive channel with its DAC still enabled therefore contributes the converted analog level for digital `0`, and local coverage now locks that distinction against the DAC-off path for the shared output pipeline.
- `NR51` routing and `NR50` master-volume changes now update the live post-HPF preview immediately on the shared T-cycle timeline instead of waiting for a frontend audio boundary; local coverage already fixes independent left/right routing, the documented `NR50` `0 -> factor 1` and `7 -> factor 8` semantics, and HPF-visible pops from output-path changes.
- The HPF now keeps explicit persistent left/right state in the APU core and is clocked from the same T-cycle timeline as the rest of the subsystem instead of being deferred to host audio code.
- The current host-facing boundary in `gb-core` is now explicit as typed post-HPF stereo samples plus a capture helper: `Apu::host_output_sample()` exposes the live post-HPF analog state, and `ApuSampleCapture` advances a frontend-chosen output cadence without feeding host sample rate or host callback timing back into the hardware model.
- Machine-level integration coverage now fixes that reading snapshots at different host-side cadences does not feed back into APU timing, mixer behavior, or HPF evolution, while local sample-capture coverage now fixes zero-rate rejection, exact long-run sample counts, and deterministic fractional-rate capture from the shared T-cycle timeline.
- Final normalization to host `float` or other sink-native sample types remains frontend-owned, so SDL, WebAssembly, libretro, or offline tooling can all consume the same core capture boundary without the APU depending on any concrete backend API.

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
- While CH1 is active, ordinary `NR12` writes should only update the readable register state and DAC status; the running envelope's latched pace/direction/initial-volume state should not be reloaded until the next CH1 trigger.
- While CH1 is active, `NR12` writes should at least model the cross-revision-consistent zombie-mode subset: writing increase mode with pace `0` increments the live current volume by `1` modulo `16`.
- Envelope progression must update CH1's internal current volume, not the readable initial-volume bits in `NR12`.
- Once an automatic envelope step would push CH1 below `0` or above `15`, the current volume should remain clamped and the envelope should stop further automatic updates until CH1 is retriggered.
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

## CH1 active-state integration baseline

- CH1 should be disabled by exactly these ordinary causes:
  - DAC disable
  - length expiry
  - CH1 sweep overflow
- CH1 should not be disabled merely because the envelope reached volume `0`.
- `NR52` bit `0` should track CH1 activity according to those rules.
- The master APU output path should consume CH1's resolved current digital output together with its DAC/active state through the channel-export boundary, while the stereo mixer itself should operate on the resulting DAC output rather than re-reading `NR10` through `NR14`.
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
- While CH2 is active, ordinary `NR22` writes should only update the readable register state and DAC status; the running envelope's latched pace/direction/initial-volume state should not be reloaded until the next CH2 trigger.
- While CH2 is active, `NR22` writes should at least model the cross-revision-consistent zombie-mode subset: writing increase mode with pace `0` increments the live current volume by `1` modulo `16`.
- Envelope progression must update CH2's internal current volume, not the readable initial-volume bits in `NR22`.
- Once an automatic envelope step would push CH2 below `0` or above `15`, the current volume should remain clamped and the envelope should stop further automatic updates until CH2 is retriggered.
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
- The master APU output path should consume CH2's resolved current digital output together with its DAC/active state through the channel-export boundary, while the stereo mixer itself should operate on the resulting DAC output rather than re-reading `NR21` through `NR24`.
- CH2 should expose distinct temporal inputs for:
  - fast pulse timing on the shared T-cycle timeline
  - slow frame-sequencer clocks for length and envelope
- CH2's resolved digital output should be a function of its internal active state, DAC state, duty position, and current envelope-derived volume rather than a fresh interpretation of MMIO register contents at mix time.
- CH2 timing integration should remain split into:
  - fast waveform/period timing on the shared T-cycle timeline
  - slow frame-sequencer clocks for length and envelope

Current branch baseline, March 30, 2026:

- `CH1` and `CH2` now own explicit pulse runtime state in the core branch baseline: duty selection, non-resetting duty-step counters, T-cycle period timers, `64`-step length counters, immediate length-enable state, and envelope timer plus current-volume state.
- `CH1` now keeps a separate sweep block with explicit timer, enable flag, and shadow-period ownership instead of collapsing sweep into generic pulse helpers or leaking it into `CH2`.
- The shared frame sequencer now clocks `CH1` / `CH2` length, `CH1` sweep, and both pulse envelopes directly on the APU timeline, while the fast pulse timers keep advancing independently each T-cycle.
- Unit and integration coverage now already guards trigger-time reloads, the "trigger resets timer but not duty step" rule, period-write delay until the current sample ends, the second sweep overflow check, extra length clocking on `NR14` / `NR24`, the low frequency-timer bits preserved on trigger, the trigger-time envelope `+1` reload quirk before an envelope step, the post-power-on first-trigger output suppression latch for both pulse channels, and `NR52` live-bit clearing through length expiry or sweep overflow on the machine timeline.
- The finer `CH1` sweep micro-timing block is now explicit in the current branch baseline instead of being hand-waved: sweep clocks can still overflow with `shift = 0`, sweep pace `0` behaves as an internal `8`-step cadence, writes that move `NR10` from pace `0` back to non-zero re-enter on the correct sweep-phase boundary, subtraction uses the documented `11`-bit two's-complement path, and clearing negate after a subtraction-mode calculation disables `CH1` immediately.
- The previously open pulse follow-up quirk is now explicit too: after `NR52` power-on, the first trigger on `CH1`/`CH2` suppresses the initial duty output until the first real duty-step advance, and an `NR52` power cycle rearms that latch instead of leaving it as accidental state. For powered startup snapshots, inactive pulse channels currently keep that latch pending as the repo's explicit inference for hidden post-power-on state.

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
- Extra length clocking on `NR34` writes should remain an explicit CH3 path rather than disappearing behind generic channel code.
- Trigger-with-length-0 should keep the documented `255` versus `256` seam explicit on the shared frame-sequencer timeline instead of being flattened into an unconditional full reload.

## CH3 wave RAM access and DMG retrigger-corruption baseline

- CH3 wave-RAM access while the channel is active should remain under an explicit hardware policy rather than being treated as always-free RAM with no side effects.
- Keep a dedicated CH3 work item for the exact wave-RAM access policy while CH3 is active, even if the first implementation leaves some fine details isolated.
- Because the project scope is still DMG-only, do not treat any current CGB-family CH3 active-wave-RAM MMIO behavior in the codebase as supported behavior; keep that contract explicitly deferred until the CGB APU lane exists.
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
- The master APU output path should consume CH3's resolved current digital output together with its DAC/active state through the channel-export boundary, while the stereo mixer itself should operate on the resulting DAC output rather than re-reading `NR30` through `NR34`.
- CH3 should expose distinct temporal inputs for:
  - fast sample timing and wave-RAM fetch progression on the shared T-cycle timeline
  - slow frame-sequencer clocks for length only
- CH3's resolved digital output should be a function of its internal active state, DAC state, buffered sample, and current `NR32` output level rather than of raw MMIO register rereads at mix time.

Current branch baseline, March 30, 2026:

- `CH3` now owns explicit wave RAM, `sample_index`, `sample_buffer`, period timer, `256`-step length counter, and immediate length-enable state in the core branch baseline instead of living as a register-only placeholder.
- The wave channel now advances on its own fast timer, keeps the buffered-sample model explicit, preserves the existing buffered sample across trigger, and only loads the next nibble when the internal sample-fetch boundary is reached.
- Trigger now carries an explicit DMG-facing startup delay before the first post-trigger wave fetch instead of treating CH3 as if the first fetch happened after only one ordinary period reload.
- `NR34` now owns explicit extra-length-clock and trigger-with-length-0 handling in the core branch baseline, including the `255` reload-visible case on first-half trigger writes instead of leaving that seam as a deferred note.
- `NR32` now behaves as CH3 digital attenuation over the buffered sample rather than as a synthetic mixer knob, and the digital mute path remains separate from DAC disable or channel inactivity.
- Unit coverage now fixes buffered-read behavior, period-write delay until the next fetch boundary, immediate `NR32` attenuation semantics, the DMG active-wave-RAM access window, and the DMG retrigger-corruption seam keyed to the internal byte position exactly `2` T-cycles before the next wave fetch. Integration coverage now ties CH3 back to `NR52` length expiry on the shared frame-sequencer timeline and to wave-RAM preservation across `NR52` power-off.
- The current explicit DMG active-wave-RAM policy is that, while CH3 is active, MMIO reads and writes only succeed on the exact T-cycle where CH3 performs its internal wave-RAM fetch; outside that fetch window, reads return `0xFF` and writes are ignored.
- CGB-family active-wave-RAM MMIO remains intentionally deferred in the current branch baseline. The code must stay architecturally ready for a model-specific policy, but the current DMG-first branch must not lock in a fake CGB contract through tests or docs before that work is implemented.
- External evidence has moved the CH3 quirk lane forward: `dmg_sound 03-trigger`, `09-wave read while on`, `10-wave trigger while on`, and `12-wave write while on` now pass in the current branch baseline, so the remaining audio red set has moved away from CH3 retrigger timing and into the still-open `CH4` / power-off lanes.

## CH4 baseline (noise / LFSR)

- CH4 should be modeled as a distinct noise-channel block rather than as generic random output or a precomputed pseudo-random table.
- At minimum, the CH4 state shape should keep explicit fields or equivalent ownership for:
  - `channel_active`
  - `dac_enabled`
  - `length_counter`
  - `length_enabled`
  - envelope timer / pace / direction / current volume
  - `lfsr_state`
  - `noise_timer`
  - decoded `NR43` state such as clock shift, width mode, and clock divider
- CH4 should explicitly not inherit pulse-only state such as duty-step progression or CH1 sweep state.

## CH4 MMIO ownership baseline

- `NR41` through `NR44` should remain owned by the CH4 block rather than by a flat APU register bank.
- `NR41` should remain write-only at the MMIO contract layer and should represent the initial length write path rather than a readable live counter.
- `NR42` should keep CH4's initial volume, envelope direction, and envelope pace distinct, reusing the ordinary envelope/DAC semantics shared with `NR12` / `NR22`.
- `NR43` should not remain an opaque stored byte; CH4 should decode it into explicit noise-generation parameters such as clock shift, width mode, and clock divider.
- `NR44` bit `7` should remain the trigger input, while bit `6` should remain the length-enable control with immediate write-time effect.

## CH4 LFSR and NR43 baseline

- CH4 should keep an explicit `lfsr_state` instead of deriving output from an abstract random seed or host RNG state.
- CH4 LFSR stepping should follow one explicit internal sequence:
  - calculate the new feedback bit from bits `0` and `1`
  - write that new bit into bit `15`
  - when short-width mode is selected, also copy that new bit into bit `7`
  - shift the register right
  - derive the resolved digital output decision from the documented output bit after that step
- CH4 digital output should not be the raw numeric LFSR value; it should resolve to either digital `0` or the current envelope-derived volume according to the LFSR output bit.
- The `15`-bit and `7`-bit modes should share the same underlying LFSR machinery, with the short-width mode emerging from the additional bit-`7` feedback path rather than from a second independent pseudo-random generator.
- `NR43` bits `7..=4` should decode as clock shift, bit `3` as width mode, and bits `2..=0` as clock divider.
- Clock divider `0` should be treated as divider `0.5` on the documented CH4 timer formula rather than as literal `0` or silently coerced to `1`.
- Clock shift values `14` and `15` should prevent CH4 from receiving LFSR clocks rather than being approximated as merely "very slow noise".
- Live writes to `NR43` should update the decoded CH4 noise parameters and affect the running channel on its own timer path rather than waiting for the next trigger.
- If a live `NR43` write moves CH4 into or out of the `14`/`15` no-clocks state, the explicit `noise_timer` should be reloaded from the new decoded `NR43` state so the channel does not later resume from a stale pre-suppression countdown.

## CH4 noise-timer baseline

- CH4 should keep an explicit `noise_timer` separate from the LFSR state itself.
- The `noise_timer` should be derived from decoded `NR43` state and should produce exactly one LFSR step whenever it expires.
- The frame sequencer must not be used as CH4's noise clock; only the fast CH4 timer path should advance the LFSR.
- Writes to `NR43` should alter CH4's effective timer configuration, not swap in a different abstract "noise texture".
- Updating `NR43` should not retroactively inject an extra LFSR tick into an in-flight timer interval; the new effective timing should apply through the explicit timer/reload path rather than by mutating past channel time.
- The explicit timer hand-off for `14`/`15` suppression transitions should therefore reload the stored `noise_timer` from the new decoded state, but should not synthesize an immediate extra LFSR step at write time.

## CH4 width-mode and lock-up baseline

- CH4 short-width mode should not be implemented as a separate lookup table; it should arise from the documented extra feedback write into bit `7` before the shift.
- Changing `NR43` width mode while CH4 is already running should affect the live LFSR state rather than only taking effect on the next trigger.
- Keep an explicit CH4 work item for the documented lock-up behavior when switching from `15`-bit mode to `7`-bit mode in certain all-ones states.
- That lock-up should arise from the real LFSR contents and width-mode transition, not from a fake external mute flag disconnected from the register state.
- Retriggering CH4 should clear the lock-up by reinitializing the LFSR back to Pan Docs' zeroed trigger state.

## CH4 DAC and trigger baseline

- CH4 `dac_enabled` should derive from `NR42 & 0xF8 != 0`.
- If the DAC is off, a trigger write to `NR44` must not activate CH4.
- If a write to `NR42` turns the DAC off, CH4 should be disabled immediately.
- `channel_active` and `dac_enabled` must remain distinct CH4 states; a DAC-enabled but inactive CH4 should still correspond to digital `0` rather than to "channel off equals DAC off".
- CH4 trigger should be represented as one explicit operation that performs the channel's trigger-time state transitions rather than as unrelated side effects scattered across MMIO, envelope, and noise helpers.
- On CH4 trigger:
  - the channel should become active if the DAC is enabled
  - the envelope timer should reset
  - current volume should become the initial volume from `NR42`
  - the LFSR state should reset to the zeroed trigger state described in Pan Docs
  - the noise timer should reload coherently from the current decoded `NR43`
  - expired length state should be restored to a valid running state
- CH4 retrigger should also serve as the explicit path that exits any current LFSR lock-up state.

## CH4 length and envelope baseline

- CH4 should keep an explicit `64`-step length counter.
- That length counter should be clocked only by the frame sequencer's `256` Hz length clock, not by the channel's fast noise timer.
- `NR44` bit `6` should enable or disable the CH4 length unit immediately on write.
- If the length counter expires while enabled, CH4 should be disabled.
- Extra length clocking on `NR44` writes should remain an explicit CH4 work item and should reuse the same general infrastructure as CH1 / CH2 rather than a parallel incompatible implementation.
- CH4 should keep envelope timer state and current volume separate from the readable contents of `NR42`.
- The envelope should be clocked from the frame sequencer's `64` Hz envelope clock.
- Envelope pace `0` should disable visible automatic envelope stepping, while still preserving the documented internal timer-reload rule that a programmed pace or period of `0` behaves as `8`.
- While CH4 is active, ordinary `NR42` writes should only update the readable register state and DAC status; the running envelope's latched pace/direction/initial-volume state should not be reloaded until the next CH4 trigger.
- While CH4 is active, `NR42` writes should at least model the cross-revision-consistent zombie-mode subset: writing increase mode with pace `0` increments the live current volume by `1` modulo `16`.
- Envelope progression must update CH4's internal current volume, not the readable initial-volume bits in `NR42`.
- Once an automatic envelope step would push CH4 below `0` or above `15`, the current volume should remain clamped and the envelope should stop further automatic updates until CH4 is retriggered.
- Reaching volume `0` through the envelope must not disable CH4 by itself.

## CH4 active-state integration and timing baseline

- CH4 should be disabled by exactly these ordinary causes:
  - DAC disable
  - length expiry
- CH4 LFSR lock-up should not be modeled as `channel_active = false`; the channel may remain logically active while the resolved output is effectively silent until retrigger.
- `NR52` bit `3` should track CH4 activity according to those rules rather than merely reflecting whether CH4 is currently audible.
- The master APU output path should consume CH4's resolved current digital output together with its DAC/active state through the channel-export boundary, while the stereo mixer itself should operate on the resulting DAC output rather than re-reading `NR41` through `NR44`.
- CH4 should expose distinct temporal inputs for:
  - fast noise-timer / LFSR timing on the shared T-cycle timeline
  - slow frame-sequencer clocks for length and envelope
- CH4's resolved digital output should be a function of its internal active state, DAC state, current LFSR output bit, and current envelope-derived volume rather than of raw MMIO register rereads at mix time.

Current branch baseline, March 30, 2026:

- `CH4` now owns explicit `NR43` decode, a fast `noise_timer`, live `lfsr_state`, envelope runtime (`initial_volume`, direction, pace, timer, current volume), and trigger-time reload of timer / envelope / LFSR instead of remaining a length-only placeholder.
- The frame sequencer now clocks both CH4 length and CH4 envelope on the shared slow-control timeline, while the fast T-cycle path advances the decoded noise timer and LFSR independently.
- The fast waveform/sample/noise timers now stay decoupled from `channel_active`: after the pulse channels have seen their first post-power-on trigger, CH1/CH2 duty stepping, CH3 sample progression, and CH4 noise/LFSR stepping continue from the shared T-cycle clock even while the channel is logically inactive or currently inaudible.
- The slow-control clocks are now explicit hardware units rather than being gated by audibility: CH1 sweep and CH1 / CH2 / CH4 envelopes continue to advance from the frame sequencer according to their own enable / pace state even if `channel_active` is already false.
- DMG power-off semantics are now explicit for the shared `NRx1` length family: on DMG, writes to `NR11/NR21/NR31/NR41` while `NR52=0` update only the internal length counters, and those counters survive the power-off interval instead of being reset to the powered-on defaults.
- External evidence has closed the CH4-facing Blargg lane needed for the current Phase `7` bring-up: `dmg_sound 02-len ctr`, `08-len ctr during power`, and `11-regs after power` now pass alongside the already-green pulse and CH3 cases.
- The remaining CH4 follow-up that used to stay implicit is now covered locally as well: unit tests lock the active 7-bit LFSR window through a live `15-bit -> 7-bit` width change into the documented all-ones short-width state without dropping `channel_active`, and verify that retrigger reloads the LFSR back to the zeroed trigger state instead of needing a fake mute flag.
- Unit coverage also now fixes the `NR43` shift-`14` / `15` hand-off explicitly: entering the no-clocks state reloads the stored `noise_timer` without clocking the LFSR, and leaving that state reloads from the new decoded timer base instead of resuming from a stale pre-suppression countdown.

## Timing / accuracy requirements

- Keep channel and frame-sequencer timing visible.
- Do not mix backend sampling concerns with hardware state evolution.
- Keep internal APU sequencing compatible with the shared T-cycle timing model, even if audio output is resampled later.
- MMIO-triggered APU events such as `NR52` power transitions and `NRx4` triggers should remain visible on the shared T-cycle timeline.
- Internal APU state should advance from the shared master clock / T-cycle timeline, not from host audio callback cadence or an ad hoc `44.1`/`48` kHz loop.
- The frame-sequencer path should derive its timing from the shared `DIV`/system-counter edge, not from a duplicate software timer hidden inside the audio backend.
- Slow frame-sequencer clocks and fast per-channel waveform timers should remain distinct in the model; do not let the frame sequencer become a surrogate sample clock.
- CH1 / CH2 / CH3 / CH4 fast waveform/sample/noise timers should continue advancing from the shared T-cycle clock even when `channel_active` is false; do not reintroduce audibility-driven gating through a software mute flag.
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
- in this repo, wire the DMG-facing external APU lane through the individual
  Blargg `dmg_sound 01..12` ROMs from `GBEmulatorShootout`; keep any later
  changes to that promoted repo-gated slice explicit instead of silently
  treating the family as permanently closed
- register semantics tests
- frame-sequencer timing tests
- direct-boot register-readback tests for the published post-boot audio snapshot when startup presets bypass firmware execution
- direct-boot continuity tests that verify the currently synthesized startup seams stay explicit, especially published `NRxx` readback, powered-off convergence, and `DIV-APU` continuity from the shared divider preset instead of silently restarting audio timing from an unrelated zero phase
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
- tests for per-channel DAC conversion where enabled-DAC digital `0..15` output maps through the documented negative-slope analog range instead of being treated as host PCM directly
- tests that DAC-off output remains distinct from "inactive channel with DAC still enabled and therefore outputting digital `0` through the DAC"
- tests for immediate `NR51` routing changes, `NR50` immediate volume changes, and the documented pop-producing DC-offset path through the HPF
- tests that the HPF keeps persistent left/right state across captured samples rather than acting as a memoryless per-sample post-process
- tests that host sample-rate changes or host-buffer sizing do not alter the core APU's internal timing, mixing semantics, HPF behavior, or pop generation
- tests for `NR10`-`NR14` ownership and MMIO semantics, including `NR13` write-only readback policy
- tests for CH1 duty-step behavior, including "retrigger resets timer but not duty step"
- tests for CH1 period-write delay where `NR13` / `NR14` changes apply after the current sample ends
- tests that CH1 trigger reloads period/envelope/sweep state but does not activate the channel while DAC is off
- tests for CH1 length expiry, CH1 envelope progression, the rule that envelope volume reaching `0` does not disable the channel, and the rule that automatic envelope updates stop after saturation until retrigger
- sweep tests covering trigger-time shadow copy, immediate overflow check, timed writeback, second overflow check, and the rule that `NR13` / `NR14` writes do not update the sweep shadow automatically
- dedicated CH1 quirk tests for period-`0`-treated-as-`8`, extra length clocking, low frequency-timer bits on trigger, the first-duty-step-after-power-on path, and the conservative live `NR12` zombie-mode increment path
- tests for `NR21`-`NR24` ownership and MMIO semantics, including `NR23` write-only readback policy
- tests for CH2 duty-step behavior, including "retrigger resets timer but not duty step"
- tests for CH2 period-write delay where `NR23` / `NR24` changes apply after the current sample ends
- tests that CH2 trigger reloads period/envelope state but does not activate the channel while DAC is off
- tests for CH2 length expiry, CH2 envelope progression, the rule that envelope volume reaching `0` does not disable the channel, and the rule that automatic envelope updates stop after saturation until retrigger
- dedicated CH2 quirk tests for envelope-timer `0 -> 8`, extra length clocking, low frequency-timer bits on trigger, the first-duty-step-after-power-on path, and the conservative live `NR22` zombie-mode increment path
- tests for `NR30`-`NR34` ownership and MMIO semantics, including `NR31` / `NR33` write-only readback policy
- tests that wave RAM remains accessible and is not cleared by `NR52` power-off
- tests for CH3 period-timer cadence at one tick every `2` dots, `32`-sample index progression, and sample-buffer reload from wave RAM
- tests for CH3 period-write delay where `NR33` / `NR34` changes apply only after the next wave-RAM read boundary
- tests that CH3 trigger reloads timer/index state but does not activate the channel while DAC is off and does not clear or refill the sample buffer automatically
- tests for CH3 length expiry, `NR32` digital output-level semantics, and the rule that `NR32` mute is not equivalent to DAC-off or channel-off
- dedicated CH3 quirk tests for digital-`0` startup state, skipped-first-sample / first-buffer behavior, explicit wave-RAM access policy while active, trigger-with-length-0 behavior, and DMG-family retrigger corruption keyed both to the exact byte-read position and to the aligned source block whenever those behaviors are implemented
- tests for `NR41`-`NR44` ownership and MMIO semantics, including `NR41` write-only readback policy and immediate `NR44` trigger/length-enable behavior
- tests for CH4 `noise_timer` cadence, live LFSR progression, and decoded `NR43` behavior including divider `0 -> 0.5`, clock-shift `14` / `15` suppressing CH4 clocks, and the live timer hand-off into / out of that suppressed state
- tests that CH4 trigger reloads envelope/LFSR/timer state but does not activate the channel while DAC is off
- tests for CH4 length expiry, CH4 envelope progression, the rule that envelope volume reaching `0` does not disable the channel, and the rule that automatic envelope updates stop after saturation until retrigger
- dedicated CH4 quirk tests for `15`-bit versus `7`-bit mode behavior, live width-mode changes, LFSR lock-up on `15 -> 7` transitions, retrigger recovery from lock-up, extra length clocking, and the conservative live `NR42` zombie-mode increment path whenever those behaviors are implemented

## Implementation notes for this repo

- Keep output backend decoupled from the emulation core.
- Favor correctness and clarity before micro-optimizations.
- Visible post-boot `NRxx` register values for `SkipBoot` should come from the centralized boot snapshot rather than ad hoc per-register reset literals spread through APU code.
- `SkipBoot` should keep all currently synthesized hidden APU startup state explicit. In the current DMG baseline that means at least the shared-divider-derived `DIV-APU` / frame-sequencer phase, powered state, visible `NRxx` ownership, wave-RAM startup policy, and the live channel-active state reconstructed from `NR52`; fuller hidden-state continuity such as HPF history, pulse duty phase, CH3 buffered-sample position, and CH4 LFSR/noise-timer continuation remains deferred and must not be described as already solved.
- When direct boot cannot prove a canonical wave RAM contents value, keep wave RAM under an explicit startup policy; a deterministic zeroed policy is acceptable for tests and tooling as long as it is documented as emulator policy rather than hardware fact.
- Before full APU timing lands, seed direct-boot `div_apu` from the same divider preset that produces visible `DIV` rather than leaving audio on an unrelated all-zero phase.
- Wave RAM accessibility policy should stay explicit and separate from the ordinary `NRxx` register bank contract.
- A shape such as `Apu { powered, div_apu, nr50, nr51, nr52, hpf_left, hpf_right, ch1, ch2, ch3, ch4 }` is a good fit for this repo's ownership model, even if names differ.
- A stage split such as `ChannelDigitalOutput -> ChannelDac -> StereoMixer -> MasterVolume -> HighPassFilter -> HostSampleBridge` is a good fit for keeping hardware semantics and host-output concerns separate, even if final type names differ.
- Each channel should expose at least:
  - current digital output
  - `dac_enabled`
  - `channel_active`
  - trigger handling
  - slow control clocks it consumes
  - its own fast timer state
- The master APU path should own DAC conversion, stereo routing, master-volume scaling, HPF state, and host-facing sample capture rather than scattering those stages across per-channel code and frontend backends.
- The stereo mixer should consume DAC outputs derived from already-resolved channel digital output and DAC state rather than peeking back into raw register storage or channel-internal waveform state to reconstruct behavior indirectly.
- Keep a clear API boundary between exact internal audio state and the later host-facing sample or resampler path, including a distinct final normalization step for host `float` / `int16` output.
- Reserve explicit follow-up work items for per-channel quirks such as extra length clocking, CH1 sweep details, CH3 retrigger/wave-RAM edge cases, CH4 lock-up, and the remaining per-revision zombie-mode matrix beyond the conservative `increase + pace 0 => volume + 1` path.
- A channel shape such as `Channel1 { active, dac_enabled, period_value, period_timer, duty, duty_step, length_counter, length_enabled, envelope, sweep }` is a good fit for keeping CH1 readable and testable, even if field names differ.
- Keep CH1 sweep logic isolated enough that trigger-time setup, timed sweep iterations, overflow checks, and shadow-register behavior can each be tested directly.
- A sibling shape such as `Channel2 { active, dac_enabled, period_value, period_timer, duty, duty_step, length_counter, length_enabled, envelope }` is a good fit for reusing the pulse-channel base without carrying sweep-only state into CH2.
- A distinct shape such as `Channel3 { active, dac_enabled, wave_ram, sample_index, sample_buffer, output_level, period_value, period_timer, length_counter, length_enabled }` is a good fit for keeping CH3 separate from pulse-channel assumptions and making wave-RAM fetch behavior directly testable.
- A distinct shape such as `Channel4 { active, dac_enabled, length_counter, length_enabled, envelope, lfsr_state, noise_timer, nr43_decode }` is a good fit for keeping CH4's LFSR-driven behavior explicit instead of hiding it behind generic "noise" helpers.

## Known pitfalls

- mixing host sample rate concerns into hardware timing
- hiding frame-sequencer behavior behind backend callbacks
- treating the APU MMIO range as a plain register array and losing write-only or mixed-field behavior
- confusing `channel_active` with `dac_enabled`
- letting the frame sequencer drive the channels' main waveform timer instead of only their slow control units
- modeling `NR50` / `NR51` as stateless mixer knobs and losing HPF/DC-offset consequences
- converting channel state directly into host PCM without an explicit DAC -> mixer -> `NR50` -> HPF pipeline
- treating `NR50` volume `0` as mute instead of the documented minimum non-zero factor
- hiding documented pops behind backend smoothing or resampler-side "click removal"
- normalizing or clipping the core's internal analog output early just to match host sample format
- treating CH3 as a pulse channel with a custom waveform and thereby losing wave RAM, buffered-sample, output-level, and retrigger-specific behavior
- treating CH4 as random noise or a cached pseudo-random table and thereby losing explicit `NR43`, LFSR, width-mode, and lock-up behavior

## Open questions

- what internal sample-capture and resampler boundary best preserves determinism and portability without feeding host cadence back into the hardware model
