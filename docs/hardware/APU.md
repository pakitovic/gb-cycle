# APU

## Scope

Own internal sound hardware state: channels, registers, frame sequencer, DAC state, mixer/master-volume state, HPF state, and the hardware-facing export boundary before host audio backends. Do not own the host audio backend itself.

## Hardware model

Keep channel behavior and frame-sequencer timing explicit. Model the APU as a digital-to-analog pipeline rather than as four channels that directly emit host samples, and keep internal hardware stepping separate from output sampling and playback.

## Interpretation guide

- Unless a section says otherwise, the bullets in this document describe the target DMG-family hardware contract for this repo's current scope.
- If a section includes repo-specific policy or local inference, label it explicitly and do not treat it as stronger than the hardware contract above it.
- Open or deferred APU work should live in [TODO.md](../TODO.md) or [ROADMAP.md](../ROADMAP.md), not as stale "future work" wording inside the baseline sections.

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
- native-CGB `PCM12` / `FF76` and `PCM34` / `FF77` read-only digital-output taps

## APU MMIO contract baseline

- The APU should expose register semantics by field, not as a generic byte bank that happens to live in `FF10-FF3F`.
- Registers documented as write-only, such as `NR13`, `NR23`, `NR31`, and `NR41`, should follow an explicit readback policy instead of echoing the last write by default.
- Mixed registers such as `NR14`, `NR24`, `NR34`, `NR44`, and especially `NR52` should keep their read-only and writable fields distinct.
- `NR52` should treat the power bit as writable control state and the channel-on flags as read-only live status.
- Powering the APU off through `NR52` should clear APU register state and make the other APU registers read-only until power is restored, except for the documented DMG-family `NRx1` length-write path; wave RAM accessibility should remain under the documented hardware policy.
- Trigger bits in the `NRx4` family should perform their channel-start side effects on the write itself.
- Write-only fields such as initial length-timer setup should remain write-only semantically, even if internal channel state later depends on them.
- Native CGB exposes `PCM12` and `PCM34` as read-only digital-output taps after channel generation and before DAC conversion: `PCM12` low/high nibbles are CH1/CH2 and `PCM34` low/high nibbles are CH3/CH4. These registers must not become writable storage, must not change channel state on reads or writes, and must stay unavailable on DMG-family models and CGB-family compatibility mode.

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
  - make those other registers read-only until power is restored, except that on DMG-family hardware `NR11`, `NR21`, `NR31`, and `NR41` should still update the internal length counters
  - leave wave RAM accessible
  - leave the `DIV-APU` counter intact rather than resetting it
- Powering the APU on through `NR52` should reset the frame sequencer so the next step is `0`, while still deriving the timing of that next step from the live shared divider source rather than from a synthetic fresh timer.
- Because the shared `DIV-APU` source is not reset by `NR52`, the first post-power-on frame-sequencer event should remain a function of the preserved live divider phase rather than of a fixed fresh-delay assumption.
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
- A channel may remain inactive while its DAC stays enabled; that should still be representable as the analog level produced by digital `0`, rather than as "DAC off".

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
- The master path should keep an explicit `VIN` lane even if the current DMG baseline feeds it a neutral analog `0`.
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
- That resolved digital output is also the source of the native-CGB `PCM12`/`PCM34` MMIO taps, so channel-output state must remain observable without bypassing or reinterpreting channel internals at the bus layer.
- The APU should keep an explicit DAC stage per channel.
- When a channel DAC is enabled, digital values `0..15` should map linearly into the analog `-1..1` range with the documented negative slope:
  - digital `0 -> analog +1`
  - digital `15 -> analog -1`
- An inactive channel with its DAC still enabled should therefore contribute the analog level corresponding to digital `0`; "inactive" must stay distinct from "DAC off".
- The DAC-off path should remain explicit rather than being faked as one more ordinary `0..15` digital conversion case.
- Pan Docs documents the per-channel DAC-off transition toward analog `0` as model-dependent; do not overclaim a final DMG fade shape until a stronger oracle exists.
- Repo-local current policy: keep a short explicit per-channel DAC-off discharge toward analog `0` on the shared T-cycle timeline, rather than collapsing DAC disable into an instantaneous zero-output step.
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
- Keep an explicit routed slot for `VIN` in the master mixer path even if the current DMG target leaves it neutral at analog `0`.
- That explicit `VIN` lane should still feed the pre-master mixer stage, but it should not bypass the all-DACs-off disconnect of the master-volume and later output stages once any residual DAC-off discharge has settled to `0`.

## HPF, DC-offset, and pops baseline

- The APU should keep one independent high-pass filter state per stereo output.
- The HPF should be placed after stereo routing and `NR50` master-volume scaling, not before them.
- The HPF must keep persistent state across captured samples; it is not a memoryless per-sample transform.
- Leaving the HPF out may exist as a temporary debug mode, but not as the target DMG-family behavior.
- HPF charge behavior should remain model-driven rather than hard-wired to one global constant: `DMG0/DMG` use the documented `0.999958` factor, while `MGB/CGB` use the more aggressive `0.998943` factor, both applied on the shared `4_194_304 Hz` T-cycle timeline.
- When all four channel DACs are off and their residual DAC-off discharges have settled to analog `0`, the master-volume path should disconnect from the output: `master_output` and post-HPF output both become `0`, and the HPF capacitor stops evolving until some channel DAC is enabled again. Routed `VIN` should not override that global disconnect.
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

- The project should keep an explicit sample-capture policy that captures the internal post-HPF stereo analog output into a host-facing stream.
- That sample-capture policy should remain independent from the T-cycle scheduler logic that advances the hardware itself.
- Changing host output rate, such as `44.1` kHz versus `48` kHz, should not require changing the internal APU hardware model.
- When the host-facing stream runs below the `4_194_304` Hz T-cycle rate, downsampling should low-pass filter the post-HPF T-cycle signal over the host interval rather than blindly choosing the last T-cycle value in that interval; a band-limited host resampler is the current preferred shape for that boundary.
- The design should leave room for replacing the host-facing resampler later without rewriting the DAC, mixer, or HPF logic.
- Conversion from the core's internal analog representation into host `float` or `int16` output should be a final representation step after HPF, not part of the hardware model itself.
- The core should keep a sufficiently precise internal analog representation so host-format conversion does not force the hardware model to clip or renormalize early.
- Repo-local current policy: the final PCM normalization step intentionally leaves fixed headroom instead of mapping the theoretical raw post-HPF peak to host full scale, so direct WAV exports stay comparable with SameBoy-style captures without changing the internal DAC/mixer/HPF model.
- Repo-local current policy also exposes typed `recorded_channel_*_pre_hpf` helpers for host tooling. Those helpers sum the selected post-DAC / `NR51` / `NR50` lanes before the shared HPF so desktop diagnostics can solo or combine `CH1..CH4` without rewriting `NR51` or mutating hardware state. The desktop frontend should then run that selected mix through a dedicated host-side HPF state that mirrors the final hardware HPF conditions, rather than treating the raw pre-HPF mix itself as a final-listenable export.

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
- Live `NR11`/`NR21` duty writes should update the length load immediately but defer the effective waveform duty until the current pulse sample finishes and the next duty-step boundary is reached; SameSuite `channel_1_duty_delay` / `channel_2_duty_delay` rely on the old duty remaining visible through `PCM12` until that sample boundary.
- Triggering CH1 from an inactive state should make the digital output begin at `0` until the first real duty-step advance; retriggering while CH1 is already active should not inject that extra suppression.
- Just after APU power-on, the first CH1/CH2 trigger should suppress the initial duty output until the first real duty-step advance, and duty clocking should remain disabled until that first trigger.
- Repo-local CGB baseline: while the native-CGB APU is powered on but a pulse channel is waiting for its first post-power-on trigger, the hidden power-on phase still advances in the CPU-visible scheduler path; an inactive CH1/CH2 trigger then applies the CGB startup delay before the first duty-step advance. At normal speed SameSuite `channel_1_delay` / `channel_2_delay` require a fixed `8` scheduler T-cycle startup delay after trigger regardless of the hidden power-on phase; in double speed the same Slice 2 speed-domain path uses a `16` scheduler T-cycle base plus the preserved hidden phase so SameSuite `channel_1_align`, `channel_2_align`, and normal-speed volume timing consume the existing timer/speed contract instead of adding a separate APU clock model. Native-CGB active retriggers keep the current duty-step phase, reset the period timer, do not reapply inactive-start output suppression, and apply only a short restart delay before the next duty-step advance: `4` scheduler T-cycles at normal speed and `8` scheduler T-cycles at double speed.

## CH1 period value and timer baseline

- `NR13` plus the low three bits of `NR14` should form CH1's `11`-bit period value.
- CH1 should keep explicit separation between the period value stored in registers and the in-flight period timer currently timing the sample.
- For the current DMG target, the pulse period timer should be clocked at `1048576` Hz, i.e. once every `4` dots.
- In native-CGB double speed, pulse generation timers consume the existing Slice `2` normal-speed domain gate so duty steps stay on the wall-clock APU domain while CPU-visible trigger phase/delay is scaled from the same current speed state used by the bus write path; this matches SameSuite `channel_1_duty` / `channel_2_duty` without forking the `DIV`/APU frame-sequencer path.
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
- Triggering CH1 should preserve the low two bits of the in-flight hardware frequency timer; in the repo's T-cycle countdown representation this means preserving the underlying timer phase, not blindly copying the low two bits of the remaining countdown value.

## CH1 length and envelope baseline

- CH1 should keep an explicit `64`-step length counter.
- That length counter should be clocked only by the frame sequencer's `256` Hz length clock, not by the channel's fast waveform timer.
- `NR14` bit `6` should enable or disable the CH1 length unit immediately on write.
- If the length counter expires while enabled, CH1 should be disabled.
- For the current DMG-family baseline, extra length clocking on `NR14` writes should remain an explicit CH1 work item; do not treat it as a negligible quirk. The documented `CGB-02` exception remains revision-specific follow-up and must not be claimed through the coarse `ConsoleModel::GameBoyColor` path.
- CH1 should keep envelope timer state and current volume separate from the readable contents of `NR12`.
- The envelope should be clocked from the frame sequencer's `64` Hz envelope clock.
- Envelope pace `0` should disable visible automatic envelope stepping, while still preserving the documented internal timer-reload rule that a programmed pace or period of `0` behaves as `8`.
- While CH1 is active, ordinary `NR12` writes should only update the readable register state and DAC status; the running envelope's latched pace/direction/initial-volume state should not be reloaded until the next CH1 trigger.
- While CH1 is active, `NR12` writes should keep the conservative DMG-family zombie-mode subset but native CGB should use the shared CGB NRx2 live-write matrix: pace `0 -> non-zero` ticks once when unlocked, `increase+pace0 -> increase+pace0` ticks once when unlocked, direction changes invert the current volume through the documented CGB paths, and the live current volume is masked to `0..=15` without reloading the latched envelope configuration until retrigger.
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
- Triggering CH2 from an inactive state should make the digital output begin at `0` until the first real duty-step advance; retriggering while CH2 is already active should not inject that extra suppression.
- Just after APU power-on, the first CH1/CH2 trigger should suppress the initial duty output until the first real duty-step advance, and duty clocking should remain disabled until that first trigger.
- Repo-local CGB baseline: CH2 shares the same native-CGB inactive-trigger startup delay, active-retrigger restart delay, and power-on phase tracking as CH1; this belongs to the shared pulse-channel primitive rather than to per-ROM or per-channel PCM patches.

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
- Triggering CH2 should preserve the low two bits of the in-flight hardware frequency timer; in the repo's T-cycle countdown representation this means preserving the underlying timer phase, not blindly copying the low two bits of the remaining countdown value.

## CH2 length and envelope baseline

- CH2 should keep an explicit `64`-step length counter.
- That length counter should be clocked only by the frame sequencer's `256` Hz length clock, not by the channel's fast waveform timer.
- `NR24` bit `6` should enable or disable the CH2 length unit immediately on write.
- If the length counter expires while enabled, CH2 should be disabled.
- For the current DMG-family baseline, extra length clocking on `NR24` writes should remain an explicit CH2 work item and should reuse the same general infrastructure as CH1 rather than a parallel incompatible implementation. The documented `CGB-02` exception remains revision-specific follow-up and must not be claimed through the coarse `ConsoleModel::GameBoyColor` path.
- CH2 should keep envelope timer state and current volume separate from the readable contents of `NR22`.
- The envelope should be clocked from the frame sequencer's `64` Hz envelope clock.
- Envelope pace `0` should disable visible automatic envelope stepping, while still preserving the documented internal timer-reload rule that a programmed pace or period of `0` behaves as `8`.
- While CH2 is active, ordinary `NR22` writes should only update the readable register state and DAC status; the running envelope's latched pace/direction/initial-volume state should not be reloaded until the next CH2 trigger.
- While CH2 is active, `NR22` writes should reuse the same shared DMG-family subset and native-CGB NRx2 live-write matrix as CH1; this is pulse-core behavior, not a channel-local PCM or per-ROM patch.
- Envelope progression must update CH2's internal current volume, not the readable initial-volume bits in `NR22`.
- Once an automatic envelope step would push CH2 below `0` or above `15`, the current volume should remain clamped and the envelope should stop further automatic updates until CH2 is retriggered.
- Reaching volume `0` through the envelope must not disable CH2 by itself.

## CH2 active-state integration and shared quirks baseline

- CH2 should be disabled by exactly these ordinary causes:
  - DAC disable
  - length expiry
- CH2 should not be disabled merely because the envelope reached volume `0`.
- `NR52` bit `1` should track CH2 activity according to those rules.
- CH2 should explicitly model the pulse-channel quirks it shares with CH1:
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
- CH3 internal wave-RAM fetches are active-channel events; while CH3 is inactive, its fast timer must not read wave RAM or refresh the sample buffer, so a buffer cleared by APU power-on remains digital `0` until a trigger starts the channel and the first post-trigger fetch occurs.

## CH3 startup and first-sample baseline

- After APU power-on, CH3 should begin with its sample buffer at digital `0`.
- CH3 startup should explicitly model the documented first-sample quirk where sample `0` is skipped when first starting the channel and the first post-trigger output is not a naive immediate replay of wave-table sample `0`.
- The APU power-on-cleared CH3 sample buffer must not be preloaded by inactive timing between `NR52` power-on, wave-RAM initialization, `NR30` DAC enable, and `NR34` trigger; SameSuite `channel_3_first_sample` relies on `PCM34` reading `0` through the startup delay and only observing the wave-table low nibble after the first real post-trigger fetch.
- A CH3 retrigger should therefore preserve the previously buffered sample until the next internal wave-RAM read occurs rather than forcing an immediate load of wave-table sample `0` or clearing the buffer automatically.

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
- For the current DMG-family baseline, extra length clocking on `NR34` writes should remain an explicit CH3 path rather than disappearing behind generic channel code. The documented `CGB-02` exception remains revision-specific follow-up and must not be claimed through the coarse `ConsoleModel::GameBoyColor` path.
- Trigger-with-length-0 should keep the documented `255` versus `256` seam explicit on the shared frame-sequencer timeline instead of being flattened into an unconditional full reload.

## CH3 wave RAM access and DMG retrigger-corruption baseline

- CH3 wave-RAM access while the channel is active should remain under an explicit hardware policy rather than being treated as always-free RAM with no side effects.
- For the current DMG-family target, CH3 active wave-RAM reads and writes should only succeed on the exact T-cycle where CH3 performs its internal wave-RAM fetch; outside that fetch window, reads should return `0xFF` and writes should be ignored.
- For CGB-family hardware, including CGB compatibility mode, CH3 active wave-RAM reads and writes should redirect to the byte CH3 is currently reading, regardless of which `FF30-FF3F` address the CPU accessed; inactive CH3 still exposes ordinary indexed wave RAM.
- CH3 DMG-family retrigger corruption should remain an explicit model-gated path rather than a side effect hidden inside generic trigger or RAM helpers.
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
  - a hidden `14`-bit noise counter plus its base-divider countdown and reload-seam flag, because some live `NR43` quirks depend on the internal counter phase rather than only on the visible `noise_timer`
  - decoded `NR43` state such as clock shift, width mode, and clock divider
- In the repo's current architecture, that ownership is grouped explicitly into a visible CH4 noise-signal block (`NR43` decode, `noise_timer`, `lfsr_state`) plus a hidden live-write phase block (noise-counter phase, divider alignment, base-divider countdown, reload-seam flag, and last traced live-write decision) so future `NR43` refactors can deepen the glitch matrix without blurring the ordinary signal path.
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
- With the repo's Pan Docs-style zeroed/XNOR LFSR representation, LFSR bit `0 = 1` selects the current envelope-derived volume and bit `0 = 0` selects digital `0`; this is equivalent to older all-ones/XOR descriptions that invert the stored LFSR output bit before applying the envelope, and SameSuite `channel_4_align` verifies the PCM34-visible polarity.
- The `15`-bit and `7`-bit modes should share the same underlying LFSR machinery, with the short-width mode emerging from the additional bit-`7` feedback path rather than from a second independent pseudo-random generator.
- `NR43` bits `7..=4` should decode as clock shift, bit `3` as width mode, and bits `2..=0` as clock divider.
- Clock divider `0` should be treated as divider `0.5` on the documented CH4 timer formula rather than as literal `0` or silently coerced to `1`.
- Clock shift values `14` and `15` should prevent CH4 from receiving LFSR clocks rather than being approximated as merely "very slow noise".
- Live writes to `NR43` should update the decoded CH4 noise parameters and affect the running channel on its own timer path rather than waiting for the next trigger.
- The repo now treats the SameBoy-guided hidden `14`-bit counter as the authoritative CH4 phase source for DMG/pre-`CGB-D`: it owns the running divider countdown, `countdown_reloaded` seam, and the bit transitions that actually step the CH4 LFSR when the channel is active.
- The visible `period_timer` field remains as a compatibility/debug mirror of the decoded `NR43` cadence, but the audible CH4 stepping now comes from the hidden counter path rather than from an independent second hardware clock.
- Live writes to `NR43` should therefore be modeled against that preserved hidden counter state, and the write-site glue should remain isolated behind a dedicated CH4 `NR43` resolver block rather than being smeared across unrelated fast-timer logic.
- Current inferred live-glitch policy is split by hardware family: DMG keeps a conservative pre-`CGB-D` staged subset cross-checked against SameBoy, while the coarse `ConsoleModel::GameBoyColor` baseline uses the direct CGB-family path that SameSuite exercises rather than inheriting DMG FF-write staging:
  - on the DMG/pre-`CGB-D` path, if the write lands on the hidden-counter reload seam, the current and previous counter values may jointly contribute one extra seam step before the ordinary staged live-write transition logic runs
  - after that seam, the DMG-family subset follows the SameBoy-guided pre-`CGB-D` structure more literally: one actual `old -> FF` write followed by one actual `FF -> new` write, with the hidden-counter-driven `old_bit / glitch_bit / new_bit` classification evaluated separately for each write
  - on CGB-family hardware, the repo's current coarse CGB profile uses the direct `old -> new` live-write path: it does not synthesize an intervening `FF` write, does not apply the pre-`CGB-D` reload-seam extra pass, and uses the current hidden counter as the effective counter even when `countdown_reloaded` is set
  - when a live write lands on the reload seam, the hidden countdown reload keeps the family-specific SameBoy alignment offsets explicit: DMG/pre-`CGB-D` uses `[2, 1, 4, 3]`, while the CGB-family direct profile uses `[2, 1, 0, 3]`; SameSuite `channel_4_freq_change` and `channel_4_frequency_alignment` cover this distinction through `PCM34`
  - the explicit pass trace still records the synthetic staged view `old_to_ff`, `ff_to_glitch_1`, optional `glitch_1_to_glitch_2`, `glitch_to_new`, and optional `low_shift_followup`, but only `old_to_ff`, `ff_to_glitch_1`, and `low_shift_followup` may mutate the LFSR in the current pre-`CGB-D` path
  - in that staged trace, `FF` remains the synthetic bus-high stage and the second-write intermediates are now derived from the `FF -> new` write itself: `glitch_1 = (0xFF & 0x7F) | (new & 0x80)` and `glitch_2 = (0xFF & 0xCF) | (new & 0x30)` only when bit 7 stays stable across `FF -> new`
  - every recorded pass exports the source/destination value, shift, selected counter bit, category, action, and LFSR state before/after the pass so desktop traces can correlate audible deltas with one explicit stage instead of a fused repo-local summary
  - inside this deterministic subset, the SameBoy-guided pre-`CGB-D` action choices are: `Category1` inert, `Category2` one plain step, `!old_bit && new_bit` one forced-short step (or the documented low-shift corruption variant), and the low-shift follow-up remains a separate final pass only when the SameBoy-style follow-up condition is met on the second `FF -> new` write
  - the hidden counter itself now follows SameBoy's pre-`CGB-D` structure more literally too: triggering CH4 no longer resets hidden-counter phase or alignment, the background counter does not run before the channel has actually started, DMG retriggers now honor the SameBoy-guided delayed-start seam when `alignment & 3 != 0`, the trigger path recomputes the hidden countdown with SameBoy-guided alignment-dependent offsets, and `effective_counter` now derives from that preserved running counter instead of from a repo-local post-trigger seed
  - SameBoy's hidden-counter `alignment` and DMG delayed-start timer live in its `2 MHz` APU domain; in this repo's `T-cycle` model those paths must therefore be translated explicitly (`alignment` advances every other `T` and the DMG delayed-start seam lasts `12` `T-cycles`, not `6`)
  - the current DMG / CGB-C-oriented action family is still deliberately conservative (`none`, `plain step`, `forced-short step`, `forced-short step + low-shift corruption`, `set feedback bits`, `step then set feedback bits`, `step then AND previous`), but the current pre-`CGB-D` branch now only uses the subset that is directly justified by the SameBoy-guided DMG / CGB-C path instead of by Zelda-tail guards
  - the old repo-local narrow-staircase Zelda-tail tuning layer is intentionally gone from this model; any remaining audible differences in Zelda should now come from the staged passes/actions themselves, not from one-off suppression rules
  - because the repo still lacks SameBoy's richer revision-specific FF-write / multi-intermediate matrix, this remains a conservative deterministic subset rather than a claim that the full DMG hardware rule is solved

## CH4 noise-timer baseline

- CH4 should keep an explicit fast-timer state in the core, but for DMG/pre-`CGB-D` the authoritative stepping source is now the SameBoy-guided hidden counter rather than an unrelated second CH4 waveform timer.
- The frame sequencer must not be used as CH4's noise clock; only the fast CH4 path should advance the LFSR.
- The hidden noise-counter countdown is therefore no longer just a live-write quirk tracker: it is the DMG/pre-`CGB-D` phase source that feeds both ordinary LFSR stepping and the `NR43` live-write seam.
- The hidden counter's `countdown_reloaded` seam is still a separate explicit state; do not collapse it away or assume a plain divider reload is enough to reproduce live `NR43` quirks.
- The hidden live-write tracker should also keep the running divider alignment explicitly, because a write that lands exactly on the hidden-counter reload seam can reload the base countdown with an alignment-dependent offset instead of the plain divider reload.
- Triggering CH4 should preserve that hidden alignment/counter history and only recompute the hidden countdown through the SameBoy-guided start helper; do not reset the hidden counter to a synthetic phase seed on every trigger.
- On the DMG family, a trigger that lands with `alignment & 3 != 0` should take the SameBoy-guided delayed-start path instead of immediately starting the channel on that same T-cycle; the hidden counter and visible LFSR start state must therefore be prepared when the delayed trigger actually fires, not when the original MMIO write lands.
- Internally, CH4 now keeps the hidden `counter_timer` and DMG `dmg_delayed_start` seam in the same `2 MHz` domain that SameBoy uses, while the rest of the project continues to tick in T-cycles. The bridge is explicit: `alignment_subphase` gates one hidden-counter / delayed-start step every two T-cycles instead of approximating that bookkeeping by doubling the countdown values.
- SameBoy also keeps CH4's hidden `alignment` timebase moving while the APU is powered off. The repo now mirrors that for DMG/pre-`CGB-D` by advancing the CH4 `alignment`/`alignment_subphase` bridge even with `NR52.7 = 0`, while still leaving the hidden counter itself idle until background counting or an active trigger starts it. When `NR52` powers back on, both `alignment` and `alignment_subphase` reset so the first powered-on CH4 start always begins from the same hidden startup phase regardless of how long the APU stayed off.
- Outside that seam, a live `NR43` write should preserve the in-flight hidden countdown rather than pretending the base-divider phase restarted immediately.
- A DMG delayed trigger is still a real trigger even if the currently programmed `NR43` shift suppresses ordinary noise timer stepping; once the delayed-start countdown reaches zero, the trigger must materialize before any early return that would otherwise skip the suppressed timer path.
- Writes to `NR43` should alter CH4's effective timer configuration, not swap in a different abstract "noise texture".
- Updating `NR43` should not retroactively inject an extra LFSR tick into an in-flight timer interval; the new effective timing should apply through the explicit timer/reload path rather than by mutating past channel time.
- The current repo policy therefore keeps the preserved hidden countdown across ordinary live writes, only reloads that countdown on the explicit seam path, and selects an explicit write-site profile: DMG uses the SameBoy-guided pre-`CGB-D` structure (`old -> FF`, then `FF -> new`, plus an optional low-shift follow-up), while CGB-family hardware uses a direct `old -> new` profile with the CGB alignment reload offsets. DMG triggers also inherit SameBoy's delayed-start seam and the divisor-`0`, `alignment == 3` visible-LFSR `0x0055` quirk, so the visible CH4 start state is prepared together with the hidden counter rather than being reset in an earlier repo-local trigger stub. The remaining richer SameBoy-style revision-specific matrices are still deferred until stronger hardware-backed evidence exists, but the pass/action seam is now explicit enough to deepen that matrix without reintroducing Zelda-tail guards or blurring the CH4 signal path.

## CH4 width-mode and lock-up baseline

- CH4 short-width mode should not be implemented as a separate lookup table; it should arise from the documented extra feedback write into bit `7` before the shift.
- Changing `NR43` width mode while CH4 is already running should affect the live LFSR state rather than only taking effect on the next trigger.
- The documented lock-up behavior when switching from `15`-bit mode to `7`-bit mode in certain all-ones states should arise from the real LFSR contents and width-mode transition, not from a fake external mute flag disconnected from the register state.
- Retriggering CH4 should clear that lock-up by reinitializing the LFSR back to Pan Docs' zeroed trigger state.

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
- For the current DMG-family baseline, extra length clocking on `NR44` writes should remain an explicit CH4 work item and should reuse the same general infrastructure as CH1 / CH2 rather than a parallel incompatible implementation. The documented `CGB-02` exception remains revision-specific follow-up and must not be claimed through the coarse `ConsoleModel::GameBoyColor` path.
- CH4 should keep envelope timer state and current volume separate from the readable contents of `NR42`.
- The envelope should be clocked from the frame sequencer's `64` Hz envelope clock.
- Envelope pace `0` should disable visible automatic envelope stepping, while still preserving the documented internal timer-reload rule that a programmed pace or period of `0` behaves as `8`.
- While CH4 is active, ordinary `NR42` writes should only update the readable register state and DAC status; the running envelope's latched pace/direction/initial-volume state should not be reloaded until the next CH4 trigger.
- While CH4 is active, `NR42` writes should reuse the same shared envelope live-write machinery as `NR12` / `NR22`, with DMG-family writes staying on the conservative subset and native-CGB writes using the CGB NRx2 matrix before CH4-specific LFSR output selection is applied.
- Envelope progression must update CH4's internal current volume, not the readable initial-volume bits in `NR42`.
- Once an automatic envelope step would push CH4 below `0` or above `15`, the current volume should remain clamped and the envelope should stop further automatic updates until CH4 is retriggered.
- Reaching volume `0` through the envelope must not disable CH4 by itself.

## CH4 active-state integration and timing baseline

- CH4 should be disabled by exactly these ordinary causes:
  - DAC disable
  - length expiry
- CH4 LFSR lock-up should not be modeled as `channel_active = false`; the channel may remain logically active while the resolved digital output becomes a fixed LFSR-selected level until retrigger, leaving analog quieting to the downstream DAC/mixer/HPF path rather than to a channel-local mute flag.
- `NR52` bit `3` should track CH4 activity according to those rules rather than merely reflecting whether CH4 is currently audible.
- The master APU output path should consume CH4's resolved current digital output together with its DAC/active state through the channel-export boundary, while the stereo mixer itself should operate on the resulting DAC output rather than re-reading `NR41` through `NR44`.
- CH4 should expose distinct temporal inputs for:
  - fast noise-timer / LFSR timing on the shared T-cycle timeline
  - slow frame-sequencer clocks for length and envelope
- CH4's resolved digital output should be a function of its internal active state, DAC state, current LFSR output bit, and current envelope-derived volume rather than of raw MMIO register rereads at mix time.

## Timing / accuracy requirements

- Keep channel and frame-sequencer timing visible.
- Keep internal APU sequencing compatible with the shared T-cycle timing model, even if audio output is captured or resampled later.
- Internal APU state should advance from the shared master clock / T-cycle timeline, not from host audio callback cadence or an ad hoc `44.1` / `48` kHz loop.
- The frame-sequencer path should derive its timing from the shared `DIV` / system-counter edge, not from a duplicate software timer hidden inside the audio backend.
- Slow frame-sequencer clocks and fast per-channel waveform/sample/noise timers should remain distinct in the model; do not let the frame sequencer become a surrogate sample clock.
- CH1 / CH2 / CH3 / CH4 fast waveform/sample/noise timers should continue advancing from the shared T-cycle clock even when `channel_active` is false; do not reintroduce audibility-driven gating through a software mute flag.
- APU power transitions, DAC-enable changes, `NR50` / `NR51` mixer changes, and `NRx4` trigger effects should all remain expressible as ordered T-cycle-visible events.
- Keep hardware state evolution separate from host-rate sample capture or resampling; the core should not depend on emitting one host sample per T-cycle.

## Dependencies

- bus and MMIO
- shared T-cycle clock or scheduler
- timer or shared divider edge source
- model/revision configuration

## Primary references

- Pan Docs APU and audio sections
- gbdev audio references and channel-specific hardware notes
- subsystem-specific hardware research and audio test ROMs where needed

## Open-source emulator references

Use `GBEmulatorShootout` as a broad maturity signal, not as an APU-specific oracle ranking. Prefer emulators with strong GB hardware fidelity and readable source when cross-checking APU behavior.

Priority order:

1. SameBoy
2. docboy
3. Gambatte
4. binjgb
5. GameRoy

## Tests

- Keep APU coverage split between local channel/unit tests in `crates/gb-core/src/apu/tests.rs` and machine/integration tests in `crates/gb-core/tests/apu.rs`.
- Keep the promoted DMG-facing external ROM lane explicit through the Blargg `dmg_sound 01..12` slice from `GBEmulatorShootout`; do not silently replace or dilute that repo-gated lane.
- Maintain shared-path coverage for:
  - write-only and mixed-register MMIO semantics
  - `NR52` power-gating, low-bit live status, and direct-boot readback / continuity
  - `DIV-APU` edge timing and frame-sequencer clocks
  - `dac_enabled` versus `channel_active`
  - DAC conversion, `NR50` / `NR51`, HPF persistence, pop-visible output changes, and host-capture independence
- Maintain CH1 / CH2 coverage for:
  - ownership and MMIO readback rules
  - duty-step timing, period-write delay, and DAC-off trigger behavior
  - envelope progression, saturation stop, and the conservative live zombie-mode increment path
  - low frequency-timer bits preserved on trigger, inactive-start output suppression, extra length clocking, and the post-power-on first-trigger quirk
  - CH1 sweep trigger-time state, timed writeback, second overflow check, and live `NR10` edge cases
- Maintain CH3 coverage for:
  - ownership and MMIO readback rules
  - buffered-sample behavior, period cadence, and period-write delay
  - DAC-off trigger behavior, `NR32` digital attenuation, and length expiry
  - DMG active wave-RAM access policy and DMG retrigger corruption windows
- Maintain CH4 coverage for:
  - ownership and MMIO readback rules
  - `noise_timer` cadence, decoded `NR43`, and width-mode behavior
  - DAC-off trigger behavior, length expiry, and envelope progression / saturation stop
  - live `15 -> 7` lock-up, retrigger recovery, extra length clocking, and the conservative live zombie-mode increment path
  - shift-`14` / `15` no-clocks suppression and the timer hand-off into / out of that state

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
- Keep genuinely unresolved APU follow-up work in [TODO.md](../TODO.md) instead of leaving solved quirks in this file behind stale "future work" wording.
- A channel shape such as `Channel1 { active, dac_enabled, period_value, period_timer, duty, duty_step, length_counter, length_enabled, envelope, sweep }` is a good fit for keeping CH1 readable and testable, even if field names differ.
- Keep CH1 sweep logic isolated enough that trigger-time setup, timed sweep iterations, overflow checks, and shadow-register behavior can each be tested directly.
- A sibling shape such as `Channel2 { active, dac_enabled, period_value, period_timer, duty, duty_step, length_counter, length_enabled, envelope }` is a good fit for reusing the pulse-channel base without carrying sweep-only state into CH2.
- A distinct shape such as `Channel3 { active, dac_enabled, wave_ram, sample_index, sample_buffer, output_level, period_value, period_timer, length_counter, length_enabled }` is a good fit for keeping CH3 separate from pulse-channel assumptions and making wave-RAM fetch behavior directly testable.
- A distinct shape such as `Channel4 { active, dac_enabled, length_counter, length_enabled, envelope, lfsr_state, noise_timer, nr43_decode }` is a good fit for keeping CH4's LFSR-driven behavior explicit instead of hiding it behind generic "noise" helpers.

## Known pitfalls

- mixing host sample-rate or resampler concerns into hardware timing
- hiding `DIV-APU` or frame-sequencer behavior behind backend callbacks or sample delivery cadence
- treating the APU MMIO range as a plain register array and thereby losing write-only, mixed-field, or power-gated behavior
- confusing `channel_active`, `dac_enabled`, and mixer audibility
- letting the frame sequencer drive the channels' fast waveform/sample/noise timers instead of only their slow control units
- treating `NR52` as a cosmetic enable bit instead of a real APU power transition
- collapsing the DAC -> mixer -> `NR50` -> HPF path into one stateless gain stage
- treating `NR50` volume `0` as mute instead of the documented minimum non-zero factor
- hiding documented pops, DC offset, or click-like behavior behind backend smoothing or early core-side normalization
- treating CH3 as a pulse or generic wavetable channel and thereby losing wave RAM, buffered-sample, output-level, and retrigger-specific behavior
- treating CH4 as random noise or a cached pseudo-random table and thereby losing explicit `NR43`, LFSR stepping, width mode, clock suppression, and lock-up behavior
