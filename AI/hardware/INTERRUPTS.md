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
- `IE` should remain owned by the interrupt controller even though it is exposed at `0xFFFF`.
- Prefer helpers such as `request_interrupt(kind)` and `clear_interrupt(kind)` alongside the routed MMIO read/write path.
- Program writes to `IF` should coexist with hardware requests without bypassing the interrupt controller's source-of-truth state.

## LCD interrupt-producer baseline

- The PPU should request the LCD STAT interrupt through the same global interrupt-controller path used by other hardware producers; the interrupt controller must not try to recompute STAT source conditions from raw `STAT`, `LY`, or mode state on its own.
- The PPU-side LCD STAT producer should already have resolved rising-edge behavior and STAT blocking before calling into the interrupt controller.
- Entering VBlank can legitimately produce both a VBlank request and an LCD STAT Mode `1` request on the same dot; these must remain distinct interrupt sources that coexist in `IF`.

## Joypad interrupt-producer baseline

- The joypad subsystem should request the joypad interrupt only when one of the visible `P1/JOYP` low-nibble bits transitions `High -> Low` after the current row-selection state has been applied.
- A raw host-input or physical-button event is not enough on its own; the request condition depends on the CPU-visible `P1` state, including whether the relevant row is currently selected.
- If both joypad rows are selected, the joypad producer must use the same combined visible-nibble semantics as `JOYP` readback; do not add a second, row-prioritized IRQ path.
- The joypad producer should hand the interrupt controller an ordinary joypad request event rather than mutating CPU dispatch flow or jumping to vector `0x60` directly.

## Timing / accuracy requirements

- Preserve ordering with CPU execution, `EI`, `DI`, `HALT`, and timer/PPU requests.
- Interrupt request and acknowledge behavior should be reasoned about on the shared T-cycle timeline.
- A pending request in `IF` should remain observable even when `IME = 0`; masking by `IME` affects CPU acceptance, not whether the request exists.
- Timer interrupt requests must remain aligned with the timer's real overflow/reload sequence rather than an oversimplified "overflow happened, so request now" shortcut.
- LCD/STAT timing should stay aligned with PPU mode transitions, including entry into Mode 2.
- Joypad interrupt timing should stay aligned with the T-cycle at which the visible `P1` low nibble actually gains a new low bit, whether that change came from an `FF00` selection write, a hardware-facing input transition, or both.
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

## Open-source emulator references

- SameBoy
- binjgb
- Mooneye GB
- GameRoy

## Tests

- Mooneye interrupt timing tests
- focused tests for priority, masking, and delayed enable behavior
- tests for `IF`/`IE` read-write behavior at `FF0F` and `FFFF`
- tests for pending-request visibility with `IME = 0`
- tests for multiple simultaneous pending requests resolving in fixed priority order
- timer interrupt timing tests that verify IF request timing relative to TIMA overflow/reload
- timer interrupt integration tests that verify CPU-visible servicing order after the request becomes pending
- LCD/STAT timing tests, including mode transitions and STAT quirk coverage when available
- tests where VBlank and LCD STAT Mode `1` requests become pending together and remain distinguishable in `IF`
- joypad interrupt tests that distinguish selected-row versus unselected-row button changes
- joypad interrupt tests that verify visible `High -> Low` detection rather than generic "button changed" behavior
- direct-boot readback tests for documented startup `IF`/`IE` values when firmware execution is bypassed

## Implementation notes for this repo

- Keep source signaling separate from CPU acknowledgement.
- A helper such as `request_interrupt(kind)` is preferred over handwritten bit-twiddling at each producer site.
- A helper such as `clear_interrupt(kind)` is also preferred over ad hoc bit masking outside the interrupt controller when software-visible acknowledge logic needs it.
- Keep the final decision to accept and dispatch an interrupt in CPU flow, even if priority selection and `IF`/`IE` ownership live here.
- Direct-boot startup values for `IF` and `IE` should be sourced from the centralized post-boot snapshot rather than inferred from CPU-local interrupt state.
- Keep the semantic ownership of `IF` and `IE` here even though bus decode must route `0xFF0F` and `0xFFFF` correctly.
- Let the PPU own the generation rules for LCD STAT requests, including rising-edge detection and DMG STAT-write quirks; the interrupt controller should only observe the resulting request events.
- Let the joypad subsystem own the `P1` visibility comparison that decides whether a joypad request happened; the interrupt controller should consume the resulting request event, not re-derive it from raw button state.

## Known pitfalls

- conflating request with acceptance
- bypassing `IF` by letting hardware call directly into CPU interrupt dispatch
- hiding delayed effects from `EI`
- decoupling STAT/LCD interrupt timing from the real PPU mode schedule
- recomputing LCD STAT source conditions in the interrupt controller instead of consuming the PPU's request events
- requesting the joypad interrupt from abstract button events rather than from visible `P1` `High -> Low` transitions
- assuming the DMG STAT write quirk applies unchanged to GBC-in-DMG-mode

## Open questions

- what the narrowest MMIO-facing API is for exposing `IF`/`IE` through the bus without leaking ad hoc bit-twiddling across the codebase
