# APU

## Scope

Own the Game Boy audio processing unit inside `gb-core`: `NR10`-`NR52`, wave RAM, CGB-family `PCM12` / `PCM34`, channel runtime state, frame sequencer, DAC state, mixer routing, master volume, HPF state, save-state serialization, and the hardware-facing sample boundary. Do not own SDL playback, host devices, desktop recording UX, or file formats; those live in the frontend docs such as [`../info/DESKTOP.md`](../info/DESKTOP.md) and [`../info/CLI.md`](../info/CLI.md).

This document states the hardware contract and repo policy for APU implementation. Timing vocabulary is shared with [`../info/TIMING-AND-ACCURACY.md`](../info/TIMING-AND-ACCURACY.md); validation policy lives in [`../TESTING.md`](../TESTING.md) and [`../info/ROM-SUITES.md`](../info/ROM-SUITES.md); source consultation order lives in [`../REFERENCES.md`](../REFERENCES.md); startup policy lives in [`BOOT-ROM.md`](BOOT-ROM.md); open follow-up belongs in [`../TODO.md`](../TODO.md) or [`../ROADMAP.md`](../ROADMAP.md), not as stale future-work prose here.

## Design rule

Model the APU as a hardware pipeline on the shared T-cycle timeline, not as four voices that directly emit host PCM. The pipeline is:

```text
channel digital output -> per-channel DAC -> NR51 routing -> NR50 master volume -> stereo HPF -> host-facing capture boundary
```

The master `Apu` owns power state, `NR50`, `NR51`, `NR52`, frame-sequencer state, output path state, channel collection, last write observation, wave-RAM startup policy, APU clock phase, and save-state payload. Channel blocks own channel-specific registers, fast timers, length/envelope/sweep units, active/DAC state, and current digital output.

## MMIO contract

- Decode APU MMIO by register owner rather than through a flat `FF10`-`FF3F` byte array.
- Preserve write-only readback policy for `NR13`, `NR23`, `NR31`, `NR33`, `NR41`, and equivalent write-only fields instead of echoing last writes.
- Keep mixed registers split by field: trigger bits are write-only actions, length-enable bits remain writable state, `NR52.7` is writable power control, and `NR52.0..=3` are read-only live channel-active bits.
- On DMG-family hardware, APU power-off through `NR52` clears ordinary APU registers and makes them read-only until power-on, except that `NR11`, `NR21`, `NR31`, and `NR41` still update internal length counters; wave RAM remains under its documented access policy.
- CGB-family mode routing exposes `PCM12` / `FF76` and `PCM34` / `FF77` as read-only digital-output taps after channel generation and before DAC conversion. `PCM12` reports CH1/CH2 low/high nibbles and `PCM34` reports CH3/CH4 low/high nibbles; DMG-family models keep those taps unavailable, while CGB-family availability follows the MMIO matrix in [`CGB.md`](CGB.md).

## Power and frame sequencer

- `NR52.7` is real APU power control, not a cosmetic mute bit.
- Power-off clears active state, ordinary registers, DAC participation, mixer output, and pending runtime-only helper state; it must not reset wave RAM or the shared `DIV-APU` source.
- Power-on resets the frame sequencer from the current timer-owned `DIV-APU` signal. DMG-family power-on resets the sequencer to step `0` and suppresses the first edge when enabling during the high half; CGB-family power-on enters the high-half phase so the next high-to-low edge clocks the envelope phase rather than a fresh step-0 length/sweep clock.
- The frame sequencer is driven by the shared divider/system-counter edge, not by a duplicate audio timer. On DMG-family normal speed, it advances from the falling edge of `DIV` bit `4` and provides length clocks at `256` Hz, CH1 sweep clocks at `128` Hz, and envelope clocks at `64` Hz.
- Scheduler order must advance the shared counter, resolve derived `DIV-APU` edges, tick APU fast timers, apply frame-sequencer clocks, and update output state without letting host audio cadence feed back into hardware timing.
- Slow frame-sequencer clocks and fast channel timers are separate. CH1/CH2 duty timers, CH3 sample fetches, and CH4 noise/LFSR timing stay on channel-owned fast paths.
- Native-CGB double speed uses the existing speed-domain gate so APU/LCD-domain work advances on the undoubled hardware cadence while CPU-visible scheduler T-cycles still model double-speed bus timing.

## Output path

- Each channel exposes a resolved digital value in the hardware `0..15` range before DAC conversion.
- Enabled DAC conversion maps digital `0` to analog `+1` and digital `15` to analog `-1` with a linear negative slope. DAC-off is an explicit state and currently discharges toward analog `0` over a short T-cycle-domain fade instead of becoming another digital sample value.
- `channel_active` and `dac_enabled` are different states. `NR52` low bits report active generation circuits, while DAC state controls analog participation; an inactive but DAC-enabled channel still contributes the analog value for digital `0`.
- `NR51` is a per-channel stereo routing matrix, not a mute or volume register. Routing changes affect the pre-HPF DC offset immediately on the shared timeline.
- `NR50` scales the routed analog sums after mixing and before HPF. Volume level `0` is the documented minimum non-zero factor and must not be treated as mute; `VIN` remains an explicit neutral lane until modeled otherwise.
- The HPF has independent persistent left/right capacitor state after `NR50`. `DMG0/DMG` use the documented slower charge factor and `MGB/CGB` use the stronger charge factor; when all channel DAC output has fully disconnected, master output and post-HPF output become `0` and the capacitor stops evolving.
- Pops from DAC enable/disable, `NR51`, or `NR50` changes should emerge from DC-offset steps plus HPF response, not from ad hoc frontend smoothing or early core-side normalization.
- `ApuHostSample` is a typed post-HPF core boundary. Host resampling, WAV/AIFC writing, SDL queueing, mute, and volume are representation/delivery concerns outside hardware state.
- `ApuSampleCapture` downsamples the post-HPF T-cycle stream without changing APU semantics. It uses integrated capture at or above the core capture clock and a band-limited windowed-sinc path below it.
- The core exposes `recorded_channel_*_pre_hpf` taps for diagnostics. These taps sum selected post-DAC / `NR51` / `NR50` lanes before HPF; frontends that solo channels must run a separate HPF state for those diagnostic exports rather than mutating `NR51` or channel state.

## Shared channel rules

- A trigger write is an ordered channel operation, not a cluster of unrelated MMIO side effects.
- A trigger can activate the channel only when that channel's DAC is enabled.
- Ordinary deactivation causes are DAC disable, length expiry, and CH1 sweep overflow; envelope volume reaching `0` does not deactivate a channel by itself.
- Length counters are clocked only by frame-sequencer length clocks. CH1/CH2/CH4 use `64`-step counters; CH3 uses a `256`-step counter.
- Extra length clocking on `NRx4` writes remains explicit for all channels. The documented `CGB-02` exception is revision-specific follow-up and must not be claimed through the coarse `ConsoleModel::GameBoyColor` path.
- Envelope state is separate from readable `NRx2` register bits. Pace `0` disables visible automatic envelope stepping while preserving the internal reload rule where programmed pace/period `0` behaves as `8`.
- DMG-family live `NRx2` writes retain the conservative zombie-mode subset. Native-CGB live `NRx2` writes use the shared CGB matrix for pace changes, direction changes, pending even-frame envelope clocks, and post-write clock-state updates.
- Save states must serialize hidden APU phase and output-path state that affects continuation, including frame-sequencer state, APU clock phase, HPF state, DAC fade state, channel fast timers, CH1 sweep pipeline state, CH3 buffered sample state, and CH4 live-write phase state.

## CH1 and CH2 pulse channels

- CH1 and CH2 share a pulse-channel primitive for duty, period timer, length, envelope, DAC state, active state, first-trigger-after-power-on behavior, CGB trigger delay, DAC-off stopped-generator latch, period-write delay, and low frequency-timer phase preservation on trigger.
- Pulse waveform state is an `8`-step duty sequence. Retriggering resets the period timer but does not reset the duty step; powering off the APU resets pulse duty state.
- Live `NR11` / `NR21` duty writes update the length load immediately but defer the effective duty change until the current pulse sample finishes and the next duty-step boundary is reached.
- Triggering from inactive state suppresses output until the first real duty-step advance; active retriggers keep current phase and apply only the model-specific restart delay.
- `NR13`/`NR14` and `NR23`/`NR24` form `11`-bit period values. Period writes normally take effect after the current sample boundary; native-CGB keeps an additional short just-sampled window where an `NRx3` or non-trigger `NRx4` write reloads the generation timer from the new period.
- CH1 owns sweep state: timer, enable flag, shadow period, delayed trigger/writeback overflow checks, restart holds, and the DMG-family recalculation/reload pipeline. CH2 must not carry sweep-only state.
- CH1 sweep addition overflow disables CH1; decreasing sweep is not a symmetric underflow shutdown path. The second overflow check after a writeback remains explicit and must not be collapsed into a later ordinary sweep tick.

## CH3 wave channel

- CH3 is a distinct wave channel, not a pulse channel with a different waveform. It owns `NR30`-`NR34`, `16` bytes of wave RAM, `32` packed 4-bit samples, `sample_index`, `sample_buffer`, output level, period timer, length counter, DAC state, and active state.
- CH3 digital output comes from the sample buffer after `NR32` attenuation, not from a fresh wave-RAM read at mix time. `NR32 = 00` mutes digitally; `01`, `10`, and `11` output unshifted, half, and quarter sample levels respectively.
- CH3 trigger does not refill the sample buffer immediately. After APU power-on the buffer starts at digital `0`; the first post-trigger visible sample follows the hardware first-sample/startup delay rather than immediately replaying wave-table sample `0`.
- CH3 fast timing runs at the wave-channel sample-fetch cadence and only reads wave RAM while the channel is active. Inactive timing must not preload the sample buffer.
- DMG-family active wave-RAM reads/writes are only honored on the exact internal fetch T-cycle; outside that window reads return `0xFF` and writes are ignored. CGB-family active wave-RAM accesses redirect to the byte currently being read, except for the AGB GB/C compatibility profile where active wave-RAM reads return `0xFF` and writes are ignored; inactive access remains ordinary indexed wave RAM.
- DMG-family retrigger corruption is explicit and model-gated. It depends on the exact internal byte-read position: bytes `0..=3` use the special first-byte overwrite, while bytes `4..=15` copy the documented aligned 4-byte block into the first four wave-RAM bytes.

## CH4 noise channel

- CH4 is a distinct LFSR/noise channel, not random output or a cached pseudo-random table. It owns `NR41`-`NR44`, length, envelope, DAC state, active state, visible noise signal state, decoded `NR43`, LFSR state, and hidden live-write phase state.
- The LFSR step calculates feedback from bits `0` and `1`, writes the feedback into bit `15`, optionally copies it into bit `7` in short-width mode, shifts right, and resolves digital output from the output bit plus current envelope volume.
- `NR43` decodes into clock shift, width mode, and divider code. Divider code `0` means the documented `0.5` divisor; shifts `14` and `15` suppress ordinary LFSR clocks rather than acting as very slow noise.
- CH4 keeps an explicit visible noise-signal block and a hidden `14`-bit live-write phase block. The hidden block tracks alignment, subphase, counter countdown, reload seam, background-counting state, delayed DMG start, and the last pass/action trace so live `NR43` behavior stays local to CH4 instead of leaking into mixer or frontend code.
- For DMG/pre-`CGB-D`, the hidden counter is the authoritative phase source for ordinary LFSR stepping and live `NR43` seams. For the current coarse CGB-family path, live `NR43` writes use a direct `old -> new` profile with CGB alignment offsets.
- DMG-family live `NR43` writes use a staged `old -> bus-high -> new` model with an optional low-shift follow-up and an explicit reload-seam pass. Each pass records source/destination value, shift, selected counter bit, action, and LFSR before/after so traces can localize audible changes without ROM-specific guards.
- CH4 trigger initializes length/envelope/LFSR/timer state through one explicit operation. DMG-family triggers that land on a non-zero hidden alignment take the delayed-start seam; the delayed trigger still materializes even when the current shift suppresses ordinary noise clocks.
- Switching between `15`-bit and `7`-bit modes must affect live LFSR state. Lock-up is a real LFSR state, not `channel_active = false`; retriggering exits lock-up by reinitializing the LFSR.

## Startup and model policy

- `SkipBoot`, `CustomBoot`, and `RealBoot` must enter the APU through the centralized startup state described in [`BOOT-ROM.md`](BOOT-ROM.md), not through scattered per-register reset literals.
- Direct-start state currently guarantees visible `NRxx` ownership, powered state, wave-RAM startup policy, `DIV-APU` / frame-sequencer phase, active mask reconstruction, and cleared runtime-only helper state. Do not claim unresolved boot-handoff state such as HPF history, pulse duty continuation, CH3 sample-buffer continuity, or CH4 LFSR/noise-timer continuation as solved.
- Wave RAM contents remain under explicit startup policy. Deterministic zeroed contents are a tooling policy unless real boot evidence selects another policy, such as the CGB RealBoot alternating pattern.
- Model and revision gates must remain explicit: DMG-family behavior, native CGB behavior, CGB compatibility behavior, and deferred revision-specific exceptions should not be collapsed into one `ConsoleModel` shortcut when observable behavior differs.

## Validation

- Keep local APU tests near the implementation under `crates/gb-core/src/apu/tests*.rs` and cross-machine/integration tests under `crates/gb-core/tests/apu*.rs` when behavior crosses public machine APIs.
- Maintain promoted ROM coverage for Blargg `dmg_sound 01..12` through the ROM-suite flow documented in [`../info/ROM-SUITES.md`](../info/ROM-SUITES.md). Add or promote CGB/APU rows only when they have stable machine-readable pass/fail or retained artifact expectations.
- Required shared coverage: MMIO readback/write gating, `NR52` power, `DIV-APU` edges, frame-sequencer clocks, channel-active versus DAC-enabled state, DAC/mixer/HPF output, host capture independence, save-state continuation, and startup state.
- Required pulse coverage: duty timing, duty-write delay, first-trigger suppression, CGB trigger delay, period-write delay, DAC-off trigger behavior, length/envelope clocks, live `NRx2` writes, extra length clocks, CH1 sweep writeback/overflow/restart seams, and preserved timer phase on trigger.
- Required CH3 coverage: wave RAM MMIO policy, buffered sample behavior, first-sample startup, period-write delay, `NR32` attenuation, DAC-off trigger behavior, length expiry, DMG active-access windows, and DMG retrigger corruption.
- Required CH4 coverage: decoded `NR43`, clock suppression, hidden counter phase, delayed trigger seam, live-write pass traces, LFSR stepping, width-mode transitions, lock-up/retrigger recovery, DAC-off trigger behavior, length/envelope clocks, and shift-`14` / `15` handoff.
- Desktop trace helpers such as `GB_CYCLE_DESKTOP_TRACE_PATH`, `GB_CYCLE_DESKTOP_CH4_NR43_TRACE_PATH`, and `GB_CYCLE_DESKTOP_CH4_STARTUP_TRACE_PATH` are investigation aids only; they must observe the same core state without changing APU timing.

## Pitfalls

- Do not clock hardware from host sample rate, host callbacks, SDL queue size, or WAV recording cadence.
- Do not hide `DIV-APU` or frame-sequencer behavior behind backend callbacks.
- Do not treat `NR50` volume `0` as mute or `NR51` as a global mute mask.
- Do not collapse DAC, routing, master volume, HPF, and host normalization into one stateless gain stage.
- Do not confuse channel-active state, DAC-enabled state, and audible output.
- Do not let frame-sequencer clocks drive fast waveform/sample/noise timers.
- Do not make CH3 a generic wavetable detached from wave-RAM MMIO and sample-buffer timing.
- Do not make CH4 random output detached from `NR43`, LFSR state, width mode, hidden counter phase, and lock-up behavior.
