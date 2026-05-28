# INTERRUPTS

## Scope

Own interrupt request state, enable state, source tracking, fixed-priority pending selection, and the CPU-visible request/acknowledge interface.

## Hardware model

Interrupts are edge- and ordering-sensitive. Keep request, mask, and acceptance logic explicit rather than scattering it across subsystems.

## Responsibilities

- represent `IF` and `IE`
- track interrupt sources
- expose a centralized interrupt request path for hardware producers
- expose fixed-priority pending selection to the CPU
- provide clear acknowledge/consume behavior to the CPU

## Registers / MMIO

- `IF` at `FF0F`
- `IE` at `FFFF`

## Map-location baseline

- `IE` being located at `0xFFFF` instead of inside `0xFF00-0xFF7F` should stay explicit in bus decode and MMIO wiring.
- `IF` should remain part of the main MMIO range while `IE` is handled as its own high-memory decode case.

## Pending interrupt baseline

- Hardware devices should request interrupts by setting the relevant bit in `IF`, not by invoking CPU dispatch logic directly.
- The effective pending mask should be derived from `IE & IF`.
- When several interrupts are pending at once, the priority order must be `VBlank > LCD STAT > Timer > Serial > Joypad`.
- The interrupt controller should expose the highest-priority pending source as a single choice for CPU dispatch rather than encouraging ad hoc priority checks in multiple places.

## `IF` / `IE` MMIO contract baseline

- `IF` and `IE` are MMIO-visible registers, but they should not be treated as generic CPU-private bytes.
- `IF` must accept both program-visible MMIO writes and asynchronous hardware requests coming from timer, PPU, serial, joypad, and other producers.
- For the current DMG-family baseline, `IF` bits `7..=5` should read back as `1`, while bits `4..=0` carry the live interrupt request state.
- `IE` should remain owned by the interrupt controller even though it is exposed at `0xFFFF`.
- Prefer helpers such as `request_interrupt(kind)` and `clear_interrupt(kind)` alongside the routed MMIO read/write path.
- Program writes to `IF` should coexist with hardware requests without bypassing the interrupt controller's source-of-truth state.
- `IE` writes are immediate even when they come from CPU stack traffic rather than an explicit `LD (a16),A`-style instruction. In particular, if interrupt-service `PC` push writes hit `0xFFFF`, the CPU must observe the updated pending set before vector commit, allowing the dispatch to cancel or retarget after the upper-byte push but not after the lower-byte push.
- Interrupt selection belongs to acceptance, and the CPU-visible acknowledge of the chosen source happens at that accept point by clearing the corresponding `IF` bit.
- The accepted source must still stay latched internally until the interrupt-service sequence resolves. If the upper-byte `PC` push into `IE` cancels or retargets the dispatch, the originally accepted but unserved source must be restored in `IF`.
- In the current DMG-family baseline, a same-source request that reappears after that accept-time acknowledge must set `IF` again and remain pending after the current interrupt service completes.

## LCD interrupt-producer baseline

- The PPU should request the LCD STAT interrupt through the same global interrupt-controller path used by other hardware producers; the interrupt controller must not try to recompute STAT source conditions from raw `STAT`, `LY`, or mode state on its own.
- The PPU-side LCD STAT producer should already have resolved rising-edge behavior and STAT blocking before calling into the interrupt controller.
- Entering VBlank can legitimately produce both a VBlank request and an LCD STAT Mode `1` request on the same dot; these must remain distinct interrupt sources that coexist in `IF`.

## Joypad interrupt-producer baseline

- The joypad subsystem should request the joypad interrupt only when one of the visible `P1/JOYP` low-nibble bits transitions `High -> Low` after the current row-selection state has been applied.
- A raw host-input or physical-button event is not enough on its own; the request condition depends on the CPU-visible `P1` state, including whether the relevant row is currently selected.
- If both joypad rows are selected, the joypad producer must use the same combined visible-nibble semantics as `JOYP` readback; do not add a second, row-prioritized IRQ path.
- The joypad producer should hand the interrupt controller an ordinary joypad request event rather than mutating CPU dispatch flow or jumping to vector `0x60` directly.

## Serial interrupt-producer baseline

- The serial subsystem should request the serial interrupt only when a transfer actually completes after the eighth serial clock shift.
- A write that sets `SC.7` must not request the interrupt immediately; it only arms or starts the transfer.
- Serial completion should clear `SC.7` and request the serial interrupt as part of the same logical transfer-complete point.
- The serial producer should hand the interrupt controller an ordinary serial request event rather than mutating CPU dispatch flow or jumping to vector `0x58` directly.

## IRQ aggregation versus CPU acceptance baseline

- Hardware producers should only emit source requests; they must not call CPU interrupt-dispatch logic directly.
- The scheduler should aggregate those source requests into `IF` after current-cycle MMIO side effects have committed, not during unrelated device-internal helper calls.
- A current-cycle Timer request produced before the CPU micro-operation can still win the next opcode slot when `IME` and `IE.Timer` are already open; the CPU-owned accept path consumes that queued Timer request before opcode fetch, and the later aggregation phase must not reassert the consumed source.
- CPU wake from `HALT` / `STOP`, pending selection, and interrupt acceptance are later CPU-owned decisions based on live `IF`, `IE`, `IME`, priority, CPU state, and the narrow same-cycle Timer queued-request path used when the reload/IRQ event already exists before the opcode fetch micro-operation.
- Timer keeps an explicit exception to any naive "request on source edge" simplification: logical TIMA overflow is not the same moment as the timer bit becoming set in `IF`.
- Serial keeps its own completion point: the request belongs to the T-cycle that completes the eighth shift and clears `SC.7`.
- Joypad keeps its own visibility rule: the request belongs only to a newly visible `High -> Low` transition in the `P1` low nibble.
- The interrupt controller still only owns the ordinary joypad `IF` request bit even for the `STOP` wake glitch family. If the CPU is waking from explicit `Stopped` state with `IME = 1`, the special bugged `0x0000` dispatch remains a CPU-owned wake/accept path layered on top of that ordinary joypad request rather than a second interrupt source stored here.
- Once the CPU accepts an interrupt, servicing it must still consume the documented DMG `20` T-cycles (`5` M-cycles) through the CPU's ordinary temporal model rather than as an immediate vector jump.
- In the current Phase `2.5` baseline for this repo, step `8` aggregation into `IF` and step `9` CPU acceptance are both wired explicitly even though the concrete producer-side request rules still land later in timer, PPU, serial, and joypad work.

## Timing / accuracy requirements

- Preserve ordering with CPU execution, `EI`, `DI`, `HALT`, and timer/PPU requests.
- Interrupt request and acknowledge behavior should be reasoned about on the shared T-cycle timeline.
- A pending request in `IF` should remain observable even when `IME = 0`; masking by `IME` affects CPU acceptance, not whether the request exists.
- Timer interrupt requests must remain aligned with the timer's real overflow/reload sequence rather than an oversimplified "overflow happened, so request now" shortcut.
- LCD/STAT timing should stay aligned with PPU mode transitions, including entry into Mode 2.
- Joypad interrupt timing should stay aligned with the T-cycle at which the visible `P1` low nibble actually gains a new low bit, whether that change came from an `FF00` selection write, a hardware-facing input transition, or both.
- Serial interrupt timing should stay aligned with the T-cycle at which the eighth serial shift completes and `SC.7` clears, whether the clocks came from the DMG internal serial clock or from externally supplied pulses.
- `IF` visibility and CPU acceptance must remain separate ordered events on the shared timeline; a producer request becoming visible in `IF` is not itself the same event as CPU service.
- When STAT behavior is implemented in detail, preserve the documented DMG-specific STAT write quirk and do not assume the same behavior on GBC running in DMG mode.

## Dependencies

- CPU
- bus/MMIO wiring
- timer
- PPU
- joypad
- serial

## Primary references

- Pan Docs interrupt sections
- AntonioND timing material
- Gekkio/Mooneye interrupt edge-case research

## Tests

- Mooneye interrupt timing tests
- focused tests for priority, masking, and delayed enable behavior
- tests for `IF`/`IE` read-write behavior at `FF0F` and `FFFF`
- tests that `IF` keeps bits `7..=5` forced high while bits `4..=0` follow live request state
- tests for pending-request visibility with `IME = 0`
- tests for multiple simultaneous pending requests resolving in fixed priority order
- timer interrupt timing tests that verify IF request timing relative to TIMA overflow/reload
- timer interrupt integration tests that verify CPU-visible servicing order after the request becomes pending
- LCD/STAT timing tests, including mode transitions and STAT quirk coverage when available
- tests where VBlank and LCD STAT Mode `1` requests become pending together and remain distinguishable in `IF`
- joypad interrupt tests that distinguish selected-row versus unselected-row button changes
- joypad interrupt tests that verify visible `High -> Low` detection rather than generic "button changed" behavior
- serial interrupt tests that verify request-on-completion rather than request-on-start
- serial interrupt tests that verify `SC.7` clear and serial request occur at the same completion point
- tests that hardware producers only request through `IF` and never bypass CPU acceptance ordering
- tests that `HALT` wake, fixed-priority selection, and later CPU acceptance remain distinguishable ordered events
- direct-boot readback tests for documented startup `IF`/`IE` values when firmware execution is bypassed

## Implementation notes for this repo

- Keep source signaling separate from CPU acknowledgement.
- A helper such as `request_interrupt(kind)` is preferred over handwritten bit-twiddling at each producer site.
- A helper such as `clear_interrupt(kind)` is also preferred over ad hoc bit masking outside the interrupt controller when software-visible acknowledge logic needs it.
- Keep the final decision to accept and dispatch an interrupt in CPU flow, even if priority selection and `IF`/`IE` ownership live here.
- Keep scheduler-phase aggregation explicit: producers should queue source requests for phase `8`, while normal CPU wake/accept observes the resulting live `IF` state during phase `9`; CPU reads of `IF` during phase `6` should not expose PPU requests that are explicitly hidden until same-cycle phase `8` aggregation, such as the LCD-restart Mode `2` pretrigger seam, and the Timer exception remains the same-cycle opcode-slot acceptance path for Timer reload requests queued before CPU phase `6`, which consumes the queued Timer source so phase `8` does not set a stale `IF` bit after acceptance.
- Direct-boot startup values for `IF` and `IE` should be sourced from the centralized post-boot snapshot rather than inferred from CPU-local interrupt state.
- Keep the semantic ownership of `IF` and `IE` here even though bus decode must route `0xFF0F` and `0xFFFF` correctly.
- Let the PPU own the generation rules for LCD STAT requests, including rising-edge detection and DMG STAT-write quirks; the interrupt controller should only observe the resulting request events.
- Let the joypad subsystem own the `P1` visibility comparison that decides whether a joypad request happened; the interrupt controller should consume the resulting request event, not re-derive it from raw button state.
- Let the serial subsystem own the transfer-complete detection that decides whether a serial request happened; the interrupt controller should consume the resulting request event, not infer completion from raw `SB` or `SC` bytes.
- In the current Phase `2.8` baseline for this repo, traces should show the interrupt controller after phase `8` aggregation and again after phase `9` CPU wake/accept evaluation, so `IF` / `IE` visibility and later service-side clear remain observable as separate ordered events on the same T-cycle timeline.

## Known pitfalls

- conflating request with acceptance
- bypassing `IF` by letting hardware call directly into CPU interrupt dispatch
- hiding delayed effects from `EI`
- decoupling STAT/LCD interrupt timing from the real PPU mode schedule
- recomputing LCD STAT source conditions in the interrupt controller instead of consuming the PPU's request events
- requesting the joypad interrupt from abstract button events rather than from visible `P1` `High -> Low` transitions
- requesting the serial interrupt from transfer-start writes instead of from real transfer completion
- assuming the DMG STAT write quirk applies unchanged to GBC-in-DMG-mode

## Open questions

- what the narrowest MMIO-facing API is for exposing `IF`/`IE` through the bus without leaking ad hoc bit-twiddling across the codebase
