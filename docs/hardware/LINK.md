# LINK

## Scope

Own passive and active external-port topologies that span more than one
console, starting with the two-console `DMG-04` Game Link Cable. Own shared
T-cycle coordination when a link topology must route clocks or data between
multiple `Machine` instances. Do not own `SB` / `SC` MMIO semantics, per-bit
serial shifting, printer command parsing, or frontend session UX.

## Responsibilities

- passive `DMG-04` cable routing between two consoles
- shared-clock propagation between internal-clock master and external-clock peer
- open-line / disconnected behavior at the cable level
- shared linked-session stepping across multiple `Machine` instances
- future active `DMG-07` adapter topology and broadcast rules

## `DMG-04` baseline

- Treat `DMG-04` as a passive cable, not as a packet protocol or active device.
- Route one console's `SOUT` byte staging toward the other console's `SIN`
  boundary.
- Preserve the serial-side "reuse the last staged outgoing byte until `SB` is
  rewritten" rule; the cable must not infer the next outgoing byte from the
  post-transfer received contents of visible `SB`.
- Route one console's internal serial clock edges toward the other console's
  external-clock ingress on the same shared T-cycle.
- If the far end is detached or not transfer-armed, treat incoming data as
  open-line high for the current DMG-focused scope, so the active master tends
  toward receiving `0xFF`.
- Pan Docs defines one side as the internal-clock master and the other as the
  external-clock slave. For the current Phase `4` baseline, simultaneous
  internal-clock transfers on both ends are treated as unsupported / undefined:
  the cable does not route a valid exchange, and each console falls back to
  open-line input while its own internal clock continues locally.
- Keep shared timing authoritative: both consoles must advance on the same
  scheduler-phase timeline before any frontend or harness presentation concerns.

## Ownership boundary

- `serial` owns `SB`, `SC`, transfer progress, bit shifting, and serial IRQ
  timing.
- `external_port` owns the per-console attachment identity and the immediate
  endpoint state presented to `serial`.
- `link` owns topology, cable routing, and shared multi-console timing.
- A linked session should expose public attach / detach operations for
  session-owned topologies such as `DMG-04`; callers should not need to mutate
  individual member machines' `external_port` attachments just to disconnect or
  reconnect a session-owned cable.
- Frontends and harnesses own player-slot UX, windows, audio muting, and host
  transport.

## Dependencies

- `hardware/SERIAL.md`
- `ARCHITECTURE.md`
- Pan Docs serial and connector sections

## Primary references

- Pan Docs — Serial Data Transfer (Link Cable)
- Pan Docs — External Connectors

## Tests

- unit tests for `DMG-04` attachment and topology validation
- integration tests for bidirectional two-console byte exchange
- integration tests for open-line/disconnected behavior
- retained chronology fixtures for at least one deterministic two-console
  transfer
