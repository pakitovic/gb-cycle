# SERIAL

## Scope

Own serial transfer registers, bit-level transfer state, clocking behavior, link-port-visible state, and the narrow signal boundary to anything connected on the far side of the handheld's external port. Do not own printer protocol state, cable / adapter topology, shared multi-console scheduling, or host networking / transport APIs.

## External-port layering baseline

Keep these layers distinct:

- `serial hardware`: the per-console controller that owns `SB`, `SC`, transfer state, bit shifting, and serial IRQ timing
- `external-port attachment`: what is physically connected to one console's port, such as nothing, a printer, a `DMG-04` cable endpoint, or a `DMG-07` adapter uplink
- `serial endpoint`: the immediate per-console signal boundary that provides incoming bits, external slave clocks, and disconnected/open-line behavior to the serial controller
- `linked session`: any owner that must coordinate multiple `Machine` instances and cable / adapter topology on one shared T-cycle timeline

The serial subsystem only owns the first layer and consumes the third. It must not silently absorb the second or fourth into ad hoc local state.

The default external-port attachment is `None` / disconnected. Printer, `DMG-04`, and `DMG-07` attachment must be selected explicitly by a frontend, harness, or tool.

## Hardware model

Keep hardware serial state explicit even if link support is stubbed initially. Model the serial port as a peripheral that shifts one bit per serial clock rather than as an atomic byte send/receive helper. Keep these concerns distinct:

- the outgoing byte currently staged for transmission
- the incoming bits accumulated so far
- how many bits have been shifted in the current transfer
- whether transfer has been requested versus whether it is actively advancing
- whether the clock source is internal or external
- whether a peer or cable endpoint is connected, disconnected, looped back, or script-driven

## Responsibilities

- `SB` and `SC` behavior
- transfer progress state
- bit-level shifting state and clocking
- serial-endpoint boundary toward the active external-port attachment or linked session
- interrupt signaling at transfer completion

## Registers / MMIO

- `SB` at `FF01`
- `SC` at `FF02`

## Serial MMIO contract baseline

- `SB` and `SC` should remain owned by the serial subsystem rather than by a generic MMIO array.
- `SB` should hold the next outgoing byte before transfer start, and during an active transfer it should reflect the live shifted state rather than staying frozen until completion.
- `SC` should remain a mixed register in the architectural sense: writable control bits plus model-dependent readback policy for non-functional fields.
- For the current DMG-family baseline, `SC` bits `6..=1` should read back as `1`, while bit `7` reflects transfer requested/in-progress state and bit `0` reflects the selected clock mode.
- For native CGB mode, `SC.1` is a functional high-speed internal-clock latch; it reads/writes as the latch only when `ConsoleModel::GameBoyColor` is running `OperatingMode::Cgb`, while DMG-family models and CGB-family `GbCompatible` mode keep `SC.1` non-functional and reading high.
- `SC.7` should express transfer requested / transfer in progress semantics, not a decorative latched bit or a "transfer finished" flag.
- `SC.0` should select internal versus external clock semantics.
- In DMG mode, `SC.1` should not expose functional high-speed behavior; keep that bit reserved for future CGB extension rather than activating undocumented DMG behavior.
- In DMG mode, `SC.0 = 1` means master mode with internal clock, while `SC.0 = 0` means slave mode waiting for external clock pulses.
- Writing `SC.7 = 1` must arm or start a transfer, but it must not complete the transfer instantly; completion still requires `8` serial clock edges.
- When serial transfer is modeled with shifting precision, `SB` reads during an active transfer should be able to reflect the in-progress shifted value rather than a frozen pre-transfer byte.

## Bit-shift transfer baseline

- The serial controller should keep explicit transfer state such as staged outgoing data, incoming data in progress, bits shifted so far, and whether transfer is merely requested or actively advancing.
- On each serial clock, one bit should leave from the MSB side of the outgoing shift path and one incoming bit should enter on the LSB side.
- `SB` should therefore evolve during the transfer as the shift register content changes bit by bit.
- After the eighth shift, `SB` should contain the fully received byte.
- If a connected peer has not loaded a new outgoing byte before the next transfer begins, the peer side should be able to resend whatever byte it still had staged; the controller should not assume every transfer starts with a freshly provided peer byte.
- That resend rule should depend on the serial-owned staged-outgoing byte, not on the post-transfer visible contents of `SB`, because `SB` contains the received byte after the previous exchange completes.

## Master and slave clock baseline

- In DMG master mode, starting a transfer with `SC.7 = 1` and `SC.0 = 1` should cause the serial subsystem to generate `8` internal serial clock pulses.
- For the current DMG target, the internal serial clock rate should be `8192` Hz.
- In native CGB master mode, the internal serial clock consumes the shared speed-domain state rather than a serial-local timer: normal speed with `SC.1 = 0` uses `8192` Hz, double speed with `SC.1 = 0` uses `16384` Hz, normal speed with `SC.1 = 1` uses `262144` Hz, and double speed with `SC.1 = 1` uses `524288` Hz.
- DMG-family master-mode clocking should follow one serial-owned free-running divider phase that is aligned to reset time rather than restarted from zero whenever software writes `SC`.
- That DMG-family serial divider phase should stay independent from timer-owned `DIV` reset behavior; writing `DIV` must not implicitly rephase the serial master clock.
- In slave mode with `SC.0 = 0`, arming the port with `SC.7 = 1` should not advance transfer progress on its own.
- In slave mode, transfer progress should occur only when external clock pulses are delivered through the peer or link-endpoint boundary.
- External serial clocks should remain allowed to arrive at non-uniform intervals; do not hard-code a fixed cadence for slave-mode progress.
- External serial clocks do not consume `SC.1`; CGB high-speed mode affects only internally clocked master transfers.
- If no external clock pulses arrive in slave mode, the transfer should remain pending indefinitely rather than timing out inside the emulation core.
- External clock pulses that arrive while slave mode is not armed with `SC.7 = 1` should be discarded rather than buffered and replayed into a later transfer.

## Peer / link-endpoint boundary baseline

- The serial core should remain separate from whatever lives on the other side of the cable.
- Use an explicit peer or link-endpoint abstraction for incoming bits, external clock pulses, and disconnected-state behavior.
- The serial controller must not assume a second emulated Game Boy is always connected.
- The immediate serial-endpoint boundary is narrower than attachment ownership: printer protocol state, `DMG-04` cable routing, `DMG-07` adapter state, and multi-machine session scheduling belong outside the local serial controller even if they ultimately drive this boundary.
- A `DMG-07` adapter reaches serial only as externally supplied slave-clock pulses and staged incoming bits. Ping packets, status tracking, transmission buffering, and adapter-port identity belong to the link topology, not to `SB` / `SC` logic.
- The peer boundary should support at least:
  - disconnected state
  - loopback or echo-style testing
  - scripted or deterministic test peers
  - future real emulator-to-emulator or transport-backed peers
- Frontends and tools should not write received bytes directly into `SB`; they should interact through the peer or link-endpoint abstraction.

## Disconnected-cable baseline

- The design should support an explicit disconnected peer state.
- For the current DMG-focused scope, a stable disconnected input should read as logical `1` on incoming bits, causing received bytes in master mode to tend toward `0xFF`.
- Do not treat disconnected behavior as proof that every historical analog edge case is already modeled; keep any finer analog disconnect transition behavior out of the initial DMG scope unless later evidence requires it.

## Serial interrupt baseline

- Serial interrupt request should happen only when the eighth shift completes, not when software writes `SC.7`.
- On transfer completion, `SC.7` should clear automatically and the serial interrupt should be requested through the shared interrupt-controller path.
- The serial subsystem should not jump to vector `0x58` directly; it should only raise the ordinary serial interrupt request.

## Scheduler integration baseline

- External serial clock pulses, peer-provided input bits, and other link-endpoint events should enter the core as timestamped events for a specific T-cycle before serial hardware advances for that cycle.
- In the current `Machine` host API, externally queued slave-clock pulses may be buffered between T-cycles, but they must cross into serial hardware only during scheduler phase `1` (`ExternalEventIngress`) so the retained trace chronology matches the shared timeline.
- Once a host pulse crosses that ingress boundary, the serial subsystem should accept it only if slave transfer state is already armed for that same shared timeline point; pulses that arrive while idle should be dropped instead of being banked for a future `SC.7` write.
- If the shared scheduler is currently held in the repo's DMG-family `STOP` gate, externally supplied serial clocks should also be dropped at ingress rather than retained and replayed after the later wake event.
- After that ingress point, serial shift work should happen as part of the shared autonomous-peripheral phase on the same T-cycle timeline.
- On the T-cycle that completes the eighth shift, the serial subsystem should update live `SB`, clear `SC.7`, and emit its completion request so the interrupt controller can aggregate it into `IF` in that same cycle.
- The scheduler must not defer serial-completion visibility to the end of an instruction, scanline, or video frame.

## Timing / accuracy requirements

- Transfer timing and completion signaling should remain explicit.
- Serial progress should remain compatible with the shared T-cycle timing model of the core.
- In DMG master mode, the `8192` Hz serial clock must derive from the emulated machine timeline rather than from host sleeps or wall-clock timers.
- In slave mode, externally supplied clock pulses must be injectable at precise points on that same shared timeline.
- `SB`, `SC`, and the serial interrupt request should become visible at the exact transfer-completion point rather than through a deferred end-of-instruction cleanup.
- Serial start and serial completion are distinct events: writing `SC.7` arms or starts transfer state on the access T-cycle, while completion visibility belongs only to the later eighth-shift T-cycle.
- For the current DMG / MGB direct-boot baseline in this repo, the serial hidden clock counter is seeded to `0xABCC` at `PC = 0x0100` so Mooneye's `boot_sclk_align` timing window matches the first post-boot internal serial edges.

## Dependencies

- bus/MMIO wiring
- interrupt controller
- T-cycle scheduler or clock source
- serial-endpoint boundary supplied by an external-port attachment or linked session owner

## Primary references

- Pan Docs serial sections

## Tests

- register semantics tests
- completion and interrupt timing tests
- tests for `SC.7` start/in-progress and completion-clears behavior
- tests for `SC.0` internal-clock versus external-clock behavior
- tests for bit-by-bit `SB` evolution during active transfer
- tests for master-mode DMG transfer timing at `8192` Hz
- tests that slave-mode transfer does not progress without externally injected clocks
- tests for disconnected-peer behavior returning incoming `1` bits and tending toward `0xFF`
- tests for loopback or scripted-peer integration without direct MMIO byte injection
- tests that serial completion requests the interrupt through `IF`
- tests that transfer-complete `SB` update, `SC.7` clear, and serial request occur together on the eighth-shift T-cycle

## Implementation notes for this repo

- Keep the hardware serial model separate from link backends.
- Let bus/MMIO wiring expose `SB` and `SC` at their mapped addresses while the serial subsystem owns transfer semantics.
- Keep one explicit serial-owned source of truth for:
  - `SB`
  - `SC`
  - transfer-active or transfer-requested state
  - selected clock mode
  - bits shifted
  - outgoing and incoming shift state or equivalent
  - master-clock timing state for DMG internal clock mode
  - optional connected peer or endpoint
- Keep the serial-owned staged outgoing byte explicit across transfer boundaries so a later transfer can resend the previous byte if software has not written a new one yet, even though visible `SB` has already been replaced by the last received byte.
- Request the serial interrupt through the shared interrupt-controller path instead of reaching into CPU interrupt state directly.
- Direct-boot startup values for `SB` and `SC` should come from the centralized post-boot snapshot rather than from serial-local guessed reset defaults.
- Direct-boot should also seed serial's hidden free-running clock phase explicitly instead of deriving it from the timer's `DIV` phase or from the moment a transfer is armed.
- Keep disconnected, loopback, scripted, and future transport-backed peers behind one narrow serial-peer boundary so the core stays transport-agnostic.
- Keep printer protocol state, `DMG-04` / `DMG-07` topology state, and linked multi-console scheduler ownership outside the serial subsystem; those owners should drive the narrow serial-endpoint boundary instead of being folded into `SB` / `SC` logic.
- The `DMG-07` adapter should still look like an external-clock endpoint from serial's point of view: the adapter owns protocol phase, packet layout, and clock cadence, while serial only shifts on pulses that cross the scheduler ingress boundary while the console is armed in slave mode.
- For `DMG-04`, the `link` session owns passive cable routing and shared-clock propagation, while each console's `external_port` attachment owns the per-console staged incoming-byte view that the serial controller consumes on the next shift boundary.
- That same `link` owner also decides when no valid cable exchange exists, for example the unsupported double-master case, and must surface open-line input rather than inventing a second valid byte-exchange path inside the serial controller.
- The peer boundary is explicit enough to distinguish disconnected input from loopback and to queue external slave-mode clock pulses on the shared timeline, while fuller scripted peers can land later without reopening MMIO ownership or transfer timing.

## Known pitfalls

- treating serial as purely frontend-defined I/O
- treating transfer as an atomic byte exchange instead of a bit-level shift process
- completing transfer immediately on `SC.7` write
- modeling received data as direct writes into `SB` from outside the serial subsystem
- coupling the serial core to a concrete host transport or network API

## Open questions

- what the narrowest peer API is for bit input, external clock pulses, staged outgoing-byte behavior, and disconnected state without overfitting to one transport
