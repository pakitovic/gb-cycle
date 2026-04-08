# Phase 7 — Audio

31. **General APU architecture**
32. **APU frame sequencer**
33. **APU channel 1**
34. **APU channel 2**
35. **APU channel 3**
36. **APU channel 4**
37. **Mixing, output, DACs, power control, and audio edge cases**

#### Goal

Build the audio subsystem as a real temporal part of the hardware, integrated with the scheduler but decoupled from each frontend's concrete audio output.

#### Modules involved

- `apu/`
- `scheduler/`
- `bus/`
- `debugger/`
- frontend audio adapters

#### Deliverables

- base APU architecture
- separate implementation of each channel
- functional frame sequencer
- a repo-managed full `blargg-dmg-curated` family that includes Blargg
  `dmg_sound 01..12` as the Phase `7` external bring-up lane
- an explicitly temporary repo-gated non-APU Blargg subset during early audio
  bring-up, followed by promotion of the full Blargg DMG family once the APU
  lane is green
- direct-boot APU startup synthesis documented coherently with the visible post-boot audio snapshot, with any remaining hidden-state gaps tracked explicitly
- final mixing
- DAC control
- power control
- audio edge cases
- clean interface between `gb-core` and frontend audio adapters

#### Phase 7 subphase plan

1. `Phase 7.0` — Validation lane and harness
   Scope: land the full curated DMG Blargg sound slice in the repo-managed
   Blargg family, keep a temporary repo-gated non-APU subset callable from CI
   and local automation until later promotion, and define the
   unit/integration plus external-ROM targets that every later audio subphase
   must satisfy.
   Done criteria: the built-in full `blargg-dmg-curated` family includes the
   upstream individual `dmg_sound 01..12` ROMs from `GBEmulatorShootout`, the
   temporary repo-gated non-APU Blargg subset remains green during bring-up,
   and the early hardening checklist plus docs name APU explicitly as an
   active bring-up lane.
2. `Phase 7.1` — Master APU MMIO, power, and ownership
   Scope: `NR50` / `NR51` / `NR52`, powered state, wave-RAM persistence across
   power-off, explicit `dac_enabled` versus `channel_active`, and one coherent
   APU-owned state shape.
   Done criteria: unit tests cover MMIO readback policy, power-off behavior,
   wave-RAM persistence, and trigger-versus-DAC semantics before channel timing
   work starts depending on them.
   Status: done in the current branch baseline. `gb-core` now keeps master
   APU control separate from per-channel register and runtime ownership,
   exposes explicit DAC-versus-active masks through the snapshot, treats
   powered-off startup state as the same observable contract as `NR52`
   power-off, and covers the resulting MMIO / power / trigger semantics with
   unit plus machine-integration tests.
3. `Phase 7.2` — `DIV-APU` and frame sequencer
   Scope: `DIV`-derived falling-edge timing, direct-boot `div_apu` entry,
   length/envelope/sweep slow clocks, and the scheduler contract for ordered
   audio inputs each T-cycle.
   Done criteria: unit tests plus integration coverage lock the `DIV`-write
   extra-tick behavior, the shared-divider ownership split, and the no-free-
   running-audio-timer rule.
   Status: done in the current branch baseline. The timer now publishes
   `DIV`-derived edge information from the shared system counter, the APU
   consumes frame-sequencer edges on the scheduler timeline while also
   responding immediately to `DIV` reset writes that produce the same falling
   edge, and `SkipBoot` now seeds `div_apu` from the same hidden divider phase
   as the timer instead of restarting audio from an unrelated zero phase.
4. `Phase 7.3` — Pulse channels (`CH1`, then `CH2`)
   Scope: explicit pulse-channel state, period timers, duty stepping, envelope,
   length, CH1 sweep, and shared pulse quirks without collapsing CH2 into a
   fake sweep-bearing copy of CH1.
   Done criteria: channel-local unit tests cover MMIO ownership and the key
   pulse quirks, while integration tests tie the pulse channels back to frame-
   sequencer clocks and `NR52` live-active bits.
   Status: done in the current branch baseline. `CH1` now owns an explicit
   pulse-plus-sweep state block, `CH2` reuses the shared pulse core without
   fake sweep state, the APU clocks both channels from the shared T-cycle plus
   frame-sequencer timeline, and unit plus machine-level integration tests now
   cover trigger reloads, duty-step persistence across retrigger, period-write
   delay, envelope/length behavior, CH1 second-overflow sweep handling,
   trigger-time extra-length and envelope `+1` quirks, preserved low timer
   bits on trigger, `NR52` power-on frame-sequencer reset plus the preserved-
   divider-phase power-on behavior, and `NR52` live-bit clearing on length
   expiry or sweep
   overflow. External evidence has also moved the remaining sweep-specific
   pulse ROMs out of the red set: `dmg_sound 03-trigger`, `04-sweep`, and
   `05-sweep_details` now pass alongside `dmg_sound 07-len sweep period sync`
   in the current branch baseline. Local pulse coverage now also closes the
   last documented hidden-state follow-up from this slice: the first
   post-power-on trigger on `CH1` / `CH2` suppresses the initial duty output
   until the first real duty-step advance, and `NR52` power cycles rearm that
   latch explicitly.
5. `Phase 7.4` — Wave channel (`CH3`)
   Scope: wave RAM ownership, sample buffer and sample index, output-level
   rules, active-wave-RAM policy, and the documented DMG retrigger-corruption
   lane.
   Done criteria: CH3 unit tests cover the buffered-read model and period-write
   delay, and integration tests cover power-off preservation plus frame-
   sequencer-coupled length behavior.
   Status: done in the current branch baseline. `CH3` now owns explicit wave
   RAM plus sample-buffer/sample-index state, period timer, output-level
   control, and `256`-step length state; the APU clocks that wave path on the
   shared T-cycle timeline and on the shared frame-sequencer length clock
   without collapsing it into the pulse-channel helpers. Unit tests now cover
   buffered reads, period-write delay, immediate `NR32` attenuation, the DMG
   active-wave-RAM access window, the `NR34` extra-length / trigger-with-
   length-0 seam, and the DMG retrigger-corruption seam keyed to the internal
   byte position exactly `2` T-cycles before the next fetch, while
   machine-level integration tests cover wave-RAM
   preservation across `NR52` power-off and CH3 length expiry through `NR52`
   bit `2`. External ROM evidence is now green on the CH3 quirk lane on top
   of that baseline: `dmg_sound 03-trigger`, `09-wave read while on`,
   `10-wave trigger while on`, and `12-wave write while on` pass in the
   current branch.
6. `Phase 7.5` — Noise channel (`CH4`)
   Scope: decoded `NR43`, explicit `noise_timer`, live LFSR progression,
   envelope/length wiring, and width-mode / lock-up behavior.
   Done criteria: CH4 unit tests cover `NR43` decode, active-state semantics,
   and retrigger recovery from lock-up; integration tests cover length/envelope
   clocks on the shared timeline.
   Status: closed in the current branch baseline for the DMG-facing scope.
   `CH4` now owns explicit `NR43` decode, `noise_timer`, `lfsr_state`,
   envelope runtime, trigger-time timer/envelope/LFSR reload, shared
   extra-length-clock handling, and `NR52` bit `3` clearing on length expiry;
   unit coverage now includes CH4 timer/LFSR/envelope seams, the Pan Docs
   zeroed trigger state, live `15-bit -> 7-bit` width-change lock-up, retrigger
   recovery from that lock-up, explicit timer hand-off into / out of the
   shift-`14` / `15` no-clocks state, inactive-channel slow-control clocking
   for CH1 sweep and CH1 / CH2 / CH4 envelopes, inactive-channel fast-timer
   continuation for CH1 / CH2 / CH3 / CH4, and DMG powered-off `NR41` length
   writes, while machine integration coverage
   includes DMG `NR41` length persistence through an `NR52` power cycle.
7. `Phase 7.6` — DAC, mixer, HPF, and host boundary
   Scope: channel digital-output ownership, per-channel DAC conversion,
   `NR51` routing, `NR50` scaling, HPF state, DC-offset / pop behavior, and the
   boundary between the T-cycle-accurate core and host-facing sample capture.
   Done criteria: unit tests cover `DAC -> mixer -> NR50 -> HPF`, and
   integration tests verify that sample-rate or buffer-size changes do not
   change core hardware semantics.
   Status: done in the current branch baseline for the core-owned boundary.
   `ApuSnapshot` now exposes an explicit output-path snapshot with per-channel
   digital output, per-channel DAC output, stereo mixer output, stereo
   master-volume output, post-HPF output, and persistent left/right HPF state.
   The core-facing host boundary is now also explicit as typed post-HPF stereo
   samples plus `ApuSampleCapture`, so frontends choose output cadence and
   final sample-format conversion without pulling SDL, browser, or libretro
   concerns into `gb-core`.
   Unit coverage now fixes DAC-enabled inactive versus DAC-off behavior,
   independent `NR51` routing, documented `NR50` factor semantics, HPF
   persistence, and immediate routing-driven pop-visible changes, while
   machine-level integration coverage fixes the current host-facing contract:
   bus writes retarget the live analog mix immediately, and host-side snapshot
   capture cadence stays non-intrusive instead of feeding back into APU timing
   or output semantics.
8. `Phase 7.7` — External closure and promotion
   Scope: drive the full DMG `dmg_sound` slice to green, promote it from the
   bring-up lane into the repo-gated subset, and record any remaining gaps
   explicitly if promotion is still blocked.
   Done criteria: the full DMG `dmg_sound 01..12` slice passes under the
   intended execution mode, retained failure artifacts remain useful, and the
   roadmap/testing docs are updated to reflect the newly promoted external
   evidence.
   Status: done in the current branch baseline. The full DMG `dmg_sound 01..12`
   slice is now promoted into the repo-gated Blargg DMG family, the built-in
   repo-gated suite no longer filters `blargg/dmg_sound`, and the
   roadmap/testing/fixture docs now describe that promoted external evidence
   explicitly instead of keeping a separate non-APU green lane alive.

#### Base APU / frame sequencer sequencing inside Phase 7

1. Establish the master APU skeleton.
   Scope: `Apu` ownership of `NR50`, `NR51`, `NR52`, powered state, left/right internal outputs, and placeholder HPF state.
   Acceptance criteria: `NR52` power on/off behavior is centralized, wave RAM remains outside the ordinary power-reset path, and the live low `NR52` bits already represent channel-active state rather than DAC-enabled state.
2. Integrate `DIV-APU` / frame-sequencer timing.
   Scope: derive `div_apu` from the shared divider timeline, using the current DMG falling-edge source on `DIV` bit `4`, emit slow clocks for length, CH1 sweep, and envelope, and leave room for coherent direct-boot entry.
   Acceptance criteria: writes to `DIV` can produce the documented extra frame-sequencer tick when the edge occurs, the APU slow clocks remain derived from the same divider source as visible `DIV`, and direct-boot audio entry can synthesize a coherent `DIV-APU` / frame-sequencer phase instead of restarting audio timing from zero.
3. Separate DAC state from channel-active state and centralize trigger behavior.
   Scope: explicit `dac_enabled` versus `channel_active`, shared trigger handling from `NRx4` bit `7`, and DAC-off forcing channel-off.
   Acceptance criteria: triggers do not activate channels whose DAC is off, DAC-disable can deactivate a live channel immediately, and `NR52` reports live active channels rather than DAC-enabled channels.
4. Build the base stereo mixer.
   Scope: per-channel routing through `NR51`, left/right master-volume scaling through `NR50`, and internal left/right analog-output accumulation.
   Acceptance criteria: stereo routing is correct, `NR50` follows the documented "0 means factor 1, 7 means factor 8" behavior, and the architecture does not confuse master volume with mute.
5. Add the output HPF layer.
   Scope: left/right HPF state in the analog-output path after mixing and master-volume scaling.
   Acceptance criteria: the pipeline has an explicit place for DC-offset and pop-sensitive behavior, and HPF presence no longer depends on frontend audio code.
6. Prepare the channel blocks without collapsing the timing model.
   Scope: stable hooks for CH1-CH4 slow clocks and fast timers, plus follow-up placeholders for channel-specific quirks and edge cases.
   Acceptance criteria: each channel can later receive its own waveform timer without changing the master frame-sequencer architecture, and known follow-up work such as extra length clocking, CH3 wave-RAM quirks, CH4 lock-up, and envelope zombie-mode remains explicitly tracked rather than implicit.

#### CH1 sequencing inside Phase 7

1. Establish CH1 state ownership and MMIO routing.
   Scope: CH1-owned `NR10`-`NR14`, explicit channel state, and write-only/read-only field policy.
   Acceptance criteria: `NR13` remains write-only, `NR14` bit `7` acts as trigger, `NR14` bit `6` acts as immediate length enable, and CH1 ownership is not split informally across generic APU helpers.
2. Implement CH1 period timer and duty stepping.
   Scope: `11`-bit period value, fast period timer, selected duty waveform, and non-resetting duty-step counter.
   Acceptance criteria: the pulse timer advances once every `4` dots on DMG, the waveform is `8` steps long, retrigger resets the timer but not duty step, and period writes take effect only after the current sample ends.
3. Implement CH1 DAC state and general trigger behavior.
   Scope: `dac_enabled`, `channel_active`, trigger-time state reload, and `NR52` bit `0` integration.
   Acceptance criteria: DAC-off disables CH1 immediately, a DAC-off trigger does not activate CH1 but still runs the documented trigger-time reload path, and CH1 trigger resets the documented period/envelope/sweep state in one explicit path.
4. Integrate CH1 length and envelope.
   Scope: `64`-step length counter, `256` Hz length clock, `64` Hz envelope clock, current-volume state, and immediate `NR14` length-enable behavior.
   Acceptance criteria: length expiry disables CH1, envelope changes current volume without mutating readable `NR12` bits, envelope volume reaching `0` does not disable CH1, and extra-length-clocking behavior is either implemented or isolated as explicit follow-up logic.
5. Implement full CH1 sweep behavior.
   Scope: shadow period, sweep timer, enabled flag, trigger-time setup, timed sweep iterations, writeback, and second overflow check.
   Acceptance criteria: trigger copies the shadow period and performs the immediate overflow check when required, sweep ticks perform writeback plus the second overflow check, and writes to `NR13` / `NR14` do not refresh the sweep shadow automatically.
6. Close CH1 quirks and fine validation.
   Scope: envelope/sweep timer-reload semantics where programmed pace or period `0` behaves as `8`, low frequency-timer bits on trigger, first-duty-step-after-power-on behavior, and any remaining documented CH1 trigger/length edge cases.
   Acceptance criteria: quirks are isolated behind explicit channel logic and tests, rather than leaking into the general APU architecture.

#### CH2 sequencing inside Phase 7

1. Establish CH2 state ownership and MMIO routing.
   Scope: CH2-owned `NR21`-`NR24`, explicit channel state, and write-only/read-only field policy without any sweep-only carryover.
   Acceptance criteria: `NR23` remains write-only, `NR24` bit `7` acts as trigger, `NR24` bit `6` acts as immediate length enable, and CH2 does not accumulate dummy sweep state just because it shares pulse-channel infrastructure with CH1.
2. Implement CH2 period timer and duty stepping.
   Scope: `11`-bit period value, fast period timer, selected duty waveform, and non-resetting duty-step counter.
   Acceptance criteria: the pulse timer advances once every `4` dots on DMG, the waveform is `8` steps long, retrigger resets the timer but not duty step, and period writes take effect only after the current sample ends.
3. Implement CH2 DAC state and general trigger behavior.
   Scope: `dac_enabled`, `channel_active`, trigger-time state reload, and `NR52` bit `1` integration.
   Acceptance criteria: DAC-off disables CH2 immediately, a DAC-off trigger does not activate CH2 but still runs the documented trigger-time reload path, and CH2 trigger resets the documented period/envelope state in one explicit path.
4. Integrate CH2 length and envelope.
   Scope: `64`-step length counter, `256` Hz length clock, `64` Hz envelope clock, current-volume state, and immediate `NR24` length-enable behavior.
   Acceptance criteria: length expiry disables CH2, envelope changes current volume without mutating readable `NR22` bits, envelope volume reaching `0` does not disable CH2, and extra-length-clocking behavior is either implemented or isolated as explicit follow-up logic using the same infrastructure as CH1.
5. Close CH2 shared pulse quirks and fine validation.
   Scope: envelope timer-reload semantics where programmed pace or period `0` behaves as `8`, low frequency-timer bits on trigger, first-duty-step-after-power-on behavior, and any remaining documented CH2 trigger/length edge cases.
   Acceptance criteria: quirks are isolated behind explicit channel logic and tests, and CH2 remains architecturally simpler than CH1 because no sweep-specific state or flow leaked into it.

#### CH3 sequencing inside Phase 7

1. Establish CH3 state ownership, MMIO routing, and wave RAM.
   Scope: CH3-owned `NR30`-`NR34`, explicit channel state, write-only/read-only field policy, and explicit `16`-byte wave RAM ownership.
   Acceptance criteria: `NR31` and `NR33` remain write-only, `NR34` bit `7` acts as trigger, `NR34` bit `6` acts as immediate length enable, wave RAM is visible through its MMIO path, and wave RAM persists across `NR52` power-off.
2. Implement CH3 period timer, sample index, and sample buffer.
   Scope: `11`-bit period value, fast period timer, `32`-sample index progression, buffered sample fetch from wave RAM, and delayed application of period writes.
   Acceptance criteria: the timer advances once every `2` dots on DMG, the sample index traverses `32` logical samples, buffered output comes from fetched wave-RAM nibbles rather than direct live reads, and period writes take effect only after the next wave-RAM read boundary.
3. Implement CH3 DAC state and general trigger behavior.
   Scope: `dac_enabled`, `channel_active`, trigger-time timer/index reload, sample-buffer preservation, and `NR52` bit `2` integration.
   Acceptance criteria: DAC-off disables CH3 immediately, a DAC-off trigger does not activate CH3 but still runs the documented timer/index reload path, retrigger does not clear or refill the sample buffer automatically, and `NR52` bit `2` reflects live CH3 activity.
4. Integrate CH3 length and output level.
   Scope: `256`-step length counter, `256` Hz length clock, `NR32` digital attenuation rules, and immediate `NR34` length-enable behavior.
   Acceptance criteria: length expiry disables CH3, `NR32` mute and shift semantics are correct, `NR32` mute is not confused with DAC-off, and trigger-with-length-0 behavior remains either implemented or isolated as explicit follow-up logic.
5. Close CH3 quirks, active-wave-RAM policy, and DMG retrigger corruption.
   Scope: digital-`0` startup state, skipped-first-sample / first-buffer behavior, wave-RAM access policy while active, and DMG-family wave-RAM corruption on retrigger.
   Acceptance criteria: quirks remain isolated behind explicit CH3 state and tests, active-wave-RAM policy is not hidden behind generic RAM behavior, and retrigger corruption distinguishes the special first-byte overwrite case for reads in bytes `0..=3` from the aligned-`4`-byte block-copy cases for reads in bytes `4..=15`.

#### CH4 sequencing inside Phase 7

1. Establish CH4 state ownership and MMIO routing.
   Scope: CH4-owned `NR41`-`NR44`, explicit channel state, and write-only/read-only field policy.
   Acceptance criteria: `NR41` remains write-only, `NR44` bit `7` acts as trigger, `NR44` bit `6` acts as immediate length enable, and CH4 ownership is not split informally across generic APU helpers.
2. Implement CH4 LFSR, `noise_timer`, and `NR43` decoding.
   Scope: explicit `lfsr_state`, explicit fast timer, decoded clock shift / width mode / divider state, and the shared `15`-bit versus `7`-bit LFSR path.
   Acceptance criteria: the ordinary `15`-bit and `7`-bit paths are both correct, divider `0` is treated as `0.5`, clock-shift values `14` and `15` suppress CH4 clocks, and live `NR43` writes alter timer behavior without mutating CH4 into a texture-swap abstraction.
3. Implement CH4 DAC state and general trigger behavior.
   Scope: `dac_enabled`, `channel_active`, trigger-time state reload, lock-up recovery on retrigger, and `NR52` bit `3` integration.
   Acceptance criteria: DAC-off disables CH4 immediately, a DAC-off trigger does not activate CH4 but still runs the documented envelope/LFSR/timer reload path, retrigger exits LFSR lock-up, and `NR52` bit `3` reflects live CH4 activity rather than mere audibility.
4. Integrate CH4 length and envelope.
   Scope: `64`-step length counter, `256` Hz length clock, `64` Hz envelope clock, current-volume state, and immediate `NR44` length-enable behavior.
   Acceptance criteria: length expiry disables CH4, envelope changes current volume without mutating readable `NR42` bits, envelope volume reaching `0` does not disable CH4, and extra-length-clocking behavior is either implemented or isolated as explicit follow-up logic using the same infrastructure as CH1 / CH2.
5. Close CH4 lock-up and fine validation.
   Scope: width-mode transition quirks, documented lock-up on `15 -> 7` in the relevant all-ones states, retrigger recovery, and any remaining CH4 trigger/length edge cases.
   Acceptance criteria: lock-up remains a consequence of real LFSR state rather than an ad hoc mute flag, retrigger recovers sound by resetting the LFSR, and the remaining CH4 quirks are isolated behind explicit channel logic and tests.

#### Final output and host-boundary sequencing inside Phase 7

1. Introduce the explicit DAC layer.
   Scope: resolved channel digital outputs in the hardware `0..15` domain, per-channel DAC conversion, and an explicit DAC-off path distinct from ordinary enabled-DAC conversion.
   Acceptance criteria: enabled-DAC conversion follows the documented negative-slope `0..15 -> -1..1` mapping, DAC-off remains distinct from "inactive channel with DAC still enabled", and the master mixer now consumes analog channel outputs instead of raw digital values.
2. Build the stereo mixer and `NR51` routing.
   Scope: left/right analog buses, per-channel routing under `NR51`, and immediate routing changes on the shared timeline.
   Acceptance criteria: each channel can route to left, right, both, or neither; `NR51` writes are immediate; and routing is modeled as analog-bus inclusion rather than as an external mute shortcut.
3. Integrate `NR50` master-volume scaling and output-side power-state coherence.
   Scope: per-output master-volume scaling, explicit `VIN` slot, and the effect of `NR52` power-off on active mix contributions.
   Acceptance criteria: `NR50` level `0` does not mute, maximum volume follows the documented highest factor, the master path exposes an explicit routed `VIN` lane even if it is currently neutral, routed `VIN` still feeds the pre-output mixer/master-volume stages without bypassing the documented all-DACs-off output disconnect, and powering the APU off removes active channel contributions from the live mix while preserving wave RAM and `DIV-APU`.
4. Add the output HPF and DC-offset / pop behavior.
   Scope: one stateful HPF per stereo output after routing and `NR50`, plus documented pop behavior from DAC-enable, `NR51`, and `NR50` changes.
   Acceptance criteria: left/right HPF state persists across captured samples, output converges back toward neutral DC offset, HPF charge policy is selected from the active console model instead of one global constant, documented pops emerge from the modeled signal path, and HPF absence remains at most a debug-only bypass.
5. Separate the T-cycle-accurate APU core from the host-facing sample/export boundary.
   Scope: explicit post-HPF analog-output exposure, sample-capture policy, host resampler/export boundary, and final normalization / format conversion outside the hardware model.
   Acceptance criteria: changing host sample rate does not change the core APU model, the core can run deterministically in tests without a real audio backend, and host-side conversion no longer owns hardware semantics such as mixing, HPF behavior, or pop generation.
6. Close final output-path integration and validation.
   Scope: end-to-end `DAC -> mixer -> NR50 -> HPF -> host-facing export boundary` behavior under dynamic routing, volume, DAC, and power changes.
   Acceptance criteria: `NR50`, `NR51`, `NR52`, and DAC-enable changes all affect the final stereo path coherently, pop-producing transitions are covered by tests, HPF behavior is deterministic, and the final host-facing export layer preserves rather than rewrites the hardware model.

#### Done criteria

- each channel is independently verifiable
- the frame sequencer coordinates the subsystem correctly
- mixing and DACs are implemented on top of a stable channel base
- direct-boot audio-visible state plus the currently modeled hidden startup seams, especially `DIV-APU`, are documented coherently instead of pretending the whole APU startup path is already a verified handoff snapshot
- the core does not depend on a concrete frontend audio backend

#### Risks if introduced too early

- effort dispersion while CPU/PPU/bus are not yet closed
- difficulty isolating bugs if the base system is still unstable
