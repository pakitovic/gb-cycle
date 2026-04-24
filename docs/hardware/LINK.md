# LINK

## Scope

Own passive and active external-port topologies that span more than one
console, starting with the two-console `DMG-04` Game Link Cable and later the
active `DMG-07` 4-Player Adapter. Own shared
T-cycle coordination when a link topology must route clocks or data between
multiple `Machine` instances. Do not own `SB` / `SC` MMIO semantics, per-bit
serial shifting, printer command parsing, or frontend session UX.

## Responsibilities

- passive `DMG-04` cable routing between two consoles
- shared-clock propagation between internal-clock master and external-clock peer
- open-line / disconnected behavior at the cable level
- shared linked-session stepping across multiple `Machine` instances
- active `DMG-07` adapter clocking, ping, and packet broadcast rules

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

## `DMG-07` adapter model

- Treat `DMG-07` as an active adapter topology, not as a passive N-way
  `DMG-04` cable.
- Keep physical adapter ports explicit. Core and test-runner APIs should name
  ports `P1..P4` or equivalent adapter-port types instead of deriving identity
  from vector position. `gb-desktop` may later map those ports to host
  `PlayerSlot` policy, but `gb-core` must not depend on frontend player-slot
  semantics.
- Model the `P1` cable as the adapter's power / uplink anchor. A powered
  Phase `9` session should therefore include a `P1` machine explicitly.
  Protocol-visible connection status is still based on observed ping replies,
  so a physically present machine can be effectively absent if it is not
  participating correctly.
- The adapter supplies the serial clock for all valid `DMG-07` exchanges.
  Connected consoles are expected to arm slave-mode transfers and consume
  external clock pulses from the adapter. Internal-clock attempts must not be
  silently treated as valid `DMG-04`-style transfers.
- The adapter has two protocol phases: a ping phase used to discover and
  acknowledge adapter-port participation, and a transmission phase used to
  broadcast packet data.
- Ping packets are four bytes long: a ping header followed by three status
  bytes. The status bytes encode the fixed physical port identity and the
  currently observed participant bits.
- Ping status is not necessarily latched for the whole four-byte packet. A
  participant that replies correctly to the early ping bytes can affect later
  status bytes in the same packet, so model status as adapter-visible state
  sampled per status byte rather than as one immutable packet snapshot.
- A console is considered protocol-connected only after the adapter observes
  the expected acknowledgement replies to the ping header and first status
  byte. Missing or malformed acknowledgements should clear that port's
  connection bit in later status bytes and packets.
- The bytes sent by software in response to the second and third status bytes
  configure transmission-phase `RATE` and `SIZE`; they should not affect the
  current ping-phase byte cadence.
- Sparse effective occupancy is valid hardware behavior. If ports `P1` and
  `P4` participate while `P2` and `P3` do not, the fourth console remains
  adapter port `P4`; it must not be compacted into `P2`.
- Transmission phase begins only through the adapter protocol sequence, not by
  attaching more cables. The adapter should emit the transmission indicator,
  then broadcast packets whose total length is `SIZE * 4` bytes. Some observed
  software sends three transition bytes followed by a non-matching filler byte,
  so compatibility tests should distinguish the minimum accepted sequence from
  the idealized four-byte sequence.
- Transmission data is buffered by adapter port and rebroadcast one packet
  later. Missing or non-participating ports contribute zero-filled slots rather
  than causing the remaining slots to be renumbered.
- During each transmission packet, the adapter should accept each port's next
  payload only during that port's `SIZE` input window; filler bytes sent during
  the rest of the packet should not overwrite buffered payload data.
- The first broadcast data after entering transmission phase starts from
  adapter-internal stale / garbage contents derived from the preceding ping
  phase; games are expected to ignore the first returned packet(s).
- Returning from transmission to ping phase is also protocol-driven and should
  be represented by explicit adapter state rather than by detaching and
  reattaching topology.
- Restart compatibility should handle at least the documented aligned
  consecutive-`0xFF` sequence and should treat shorter observed variants as
  compatibility research until tests prove the exact hardware threshold.
- Adapter clock periods and packet delays belong to the `link` topology and
  must be expressed on the shared T-cycle timeline. Do not use frontend timers,
  wall-clock sleeps, or serial-local fixed-cadence assumptions for the adapter.

## Ownership boundary

- `serial` owns `SB`, `SC`, transfer progress, bit shifting, and serial IRQ
  timing.
- `external_port` owns the per-console attachment identity and the immediate
  endpoint state presented to `serial`.
- `link` owns topology, cable routing, and shared multi-console timing.
- A linked session should expose public attach / detach operations for
  session-owned topologies such as `DMG-04` or `DMG-07`; callers should not
  need to mutate individual member machines' `external_port` attachments just
  to disconnect or reconnect a session-owned cable or adapter.
- Frontends and harnesses own player-slot UX, windows, audio muting, and host
  transport.

## Dependencies

- `hardware/SERIAL.md`
- `ARCHITECTURE.md`
- Pan Docs serial and connector sections
- Pan Docs 4-Player Adapter section

## Primary references

- Pan Docs — Serial Data Transfer (Link Cable)
- Pan Docs — External Connectors
- Pan Docs — 4-Player Adapter
- Dan Docs — DMG-07 4-Player Adapter
- Shonumi, "Edge of Emulation: Game Boy 4-Player Adapter"

## Tests

- unit tests for `DMG-04` attachment and topology validation
- integration tests for bidirectional two-console byte exchange
- integration tests for open-line/disconnected behavior
- retained chronology fixtures for at least one deterministic two-console
  transfer
- unit tests for `DMG-07` adapter-port validation, sparse occupancy, ping
  status, protocol phase transitions, one-packet transmission delay, and ping
  restart
- integration tests for deterministic `DMG-07` 2-, 3-, and 4-console sessions
  before desktop UX consumes the adapter
