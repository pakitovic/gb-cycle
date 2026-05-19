# LINK

## Scope

Own passive and active external-port topologies that span more than one console, including the two-console `DMG-04` Game Link Cable, the active `DMG-07` 4-Player Adapter, and the native-CGB IR optical pair. Own shared T-cycle coordination when a link topology must route clocks, data, or optical emitter state between multiple `Machine` instances. Do not own `SB` / `SC` MMIO semantics, per-bit serial shifting, `RP` / `FF56` sensor state, printer command parsing, or frontend session UX.

## Responsibilities

- passive `DMG-04` cable routing between two consoles
- shared-clock propagation between internal-clock master and external-clock peer
- open-line / disconnected behavior at the cable level
- shared linked-session stepping across multiple `Machine` instances
- active `DMG-07` adapter clocking, ping, and packet broadcast rules
- native-CGB IR optical routing between exactly two CGB machines

## `DMG-04` baseline

- Treat `DMG-04` as a passive cable, not as a packet protocol or active device.
- Route one console's `SOUT` byte staging toward the other console's `SIN` boundary.
- Preserve the serial-side "reuse the last staged outgoing byte until `SB` is rewritten" rule; the cable must not infer the next outgoing byte from the post-transfer received contents of visible `SB`.
- Route one console's internal serial clock edges toward the other console's external-clock ingress on the same shared T-cycle.
- If the far end is detached or not transfer-armed, treat incoming data as open-line high for the current DMG-focused scope, so the active master tends toward receiving `0xFF`.
- Pan Docs defines one side as the internal-clock master and the other as the external-clock slave. Current project policy treats simultaneous internal-clock transfers on both ends as unsupported / undefined: the cable does not route a valid exchange, and each console falls back to open-line input while its own internal clock continues locally.
- Keep shared timing authoritative: both consoles must advance on the same scheduler-phase timeline before any frontend or harness presentation concerns.

## `DMG-07` adapter model

- Treat `DMG-07` as an active adapter topology, not as a passive N-way `DMG-04` cable.
- Keep physical adapter ports explicit. Core and test-runner APIs should name ports `P1..P4` or equivalent adapter-port types instead of deriving identity from vector position. `gb-desktop` may later map those ports to host `PlayerSlot` policy, but `gb-core` must not depend on frontend player-slot semantics.
- Model the `P1` cable as the adapter's power / uplink anchor. A powered `DMG-07` topology should therefore include a `P1` machine explicitly. Protocol-visible connection status is still based on observed ping replies, so a physically present machine can be effectively absent if it is not participating correctly.
- The adapter supplies the serial clock for all valid `DMG-07` exchanges. Connected consoles are expected to arm slave-mode transfers and consume external clock pulses from the adapter. Internal-clock attempts must not be silently treated as valid `DMG-04`-style transfers.
- The adapter has two protocol phases: a ping phase used to discover and acknowledge adapter-port participation, and a transmission phase used to broadcast packet data.
- Ping packets are four bytes long: a ping header followed by three status bytes. The status bytes encode the fixed physical port identity and the currently observed participant bits.
- Ping status is not necessarily latched for the whole four-byte packet. A participant that replies correctly to the early ping bytes can affect later status bytes in the same packet, so model status as adapter-visible state sampled per status byte rather than as one immutable packet snapshot.
- A console is considered protocol-connected only after the adapter observes the expected acknowledgement replies to the ping header and first status byte. Missing or malformed acknowledgements should clear that port's connection bit in later status bytes and packets.
- The bytes sent by software in response to the second and third status bytes configure `RATE` and `SIZE`. `RATE` does not change the ping byte transfer cadence, but its low nibble changes the delay before the next ping packet.
- Ping packets use clustered serial byte transfers: bits are clocked with the adapter's short serial-clock period, bytes are separated by a small inter-byte delay, and packets are separated by a much longer delay. Do not approximate ping as one evenly spaced byte stream.
- Commercial software may present the ping reply bytes as a continuous serial-output stream rather than as values that line up perfectly with the adapter's internal byte index. The adapter model should therefore preserve the last accepted connection mask, `RATE`, and `SIZE` while the `P1` `0xAA` transition marker is being observed, instead of treating that marker packet as a malformed ping that disconnects every port or overwrites the previously configured transfer parameters.
- Sparse effective occupancy is valid hardware behavior. If ports `P1` and `P4` participate while `P2` and `P3` do not, the fourth console remains adapter port `P4`; it must not be compacted into `P2`.
- Transmission phase begins only through the adapter protocol sequence, not by attaching more cables. The adapter should emit the transmission indicator, then broadcast packets whose total length is `SIZE * 4` bytes. Some observed software sends three transition bytes followed by a non-matching filler byte, so compatibility tests should distinguish the minimum accepted sequence from the idealized four-byte sequence.
- Transmission applies the master-selected `RATE` in two parts: the high nibble stretches delay between packet bytes, while the low nibble sets a minimum total packet period. Games such as `F-1 Race` use both halves (`0x28`), so the adapter model must not treat `RATE` as only a per-bit clock divider.
- Transmission data is buffered by adapter port and rebroadcast one packet later. Missing or non-participating ports contribute zero-filled slots rather than causing the remaining slots to be renumbered.
- Model transmission buffering as the adapter's own double packet ring: while one `SIZE * 4` packet is being broadcast, the next packet is filled in the opposite half of the ring. GBE+'s working `DMG-07` model and commercial `F-1 Race` behavior both point to a one-byte pipeline at this boundary: the byte at packet position `0` is the packet-leading broadcast transfer, and the next `SIZE` transfers are committed as offsets `0..SIZE-1` for each physical port in the next packet.
- During each transmission packet, the adapter should therefore accept each port's next payload only during that pipelined `SIZE`-byte input window; filler bytes sent during the rest of the packet should not overwrite buffered payload data.
- The first broadcast data after entering transmission phase starts from adapter-internal stale / garbage contents derived from the preceding ping phase; games are expected to ignore the first returned packet(s).
- Returning from transmission to ping phase is also protocol-driven and should be represented by explicit adapter state rather than by detaching and reattaching topology.
- Restart compatibility should handle at least the documented aligned consecutive-`0xFF` sequence and should treat shorter observed variants as compatibility research until tests prove the exact hardware threshold.
- Adapter clock periods and packet delays belong to the `link` topology and must be expressed on the shared T-cycle timeline. Do not use frontend timers, wall-clock sleeps, or serial-local fixed-cadence assumptions for the adapter.

## CGB infrared pair model

- Treat CGB IR as an optical topology, not as a serial/link-cable attachment and not as an `external_port` endpoint.
- The first implemented topology is exactly two machines in native CGB mode. Non-CGB models and CGB-family compatibility mode keep `RP` unavailable through the CGB MMIO gate, so attaching the topology must not make them participate electrically.
- At the linked-session boundary, sample each machine's `RP` emitter latch during `ExternalEventIngress`, route it through the explicit CGB IR optical delay line, and apply the delayed optical input to the opposite machine before per-machine phase execution begins.
- The default CGB-to-CGB optical propagation delay is a provisional `80` T-cycle topology value: Shonumi's GBE+ Super Mario Bros. DX investigation found that instantaneous delivery is too fast for several commercial IR protocols, while local validation currently covers Pokémon Gold/Silver/Crystal Mystery Gift, Super Mario Bros. Deluxe, Pokémon Trading Card Game, Donkey Kong Country, Pokémon Pinball, and Perfect Dark. Keep this delay in the topology rather than in `RP` MMIO readback so the bus-owned sensor state remains the owner of read enable, self-emitter visibility, warmup, and fade; desktop investigation builds may override the delay to sweep title behavior, but such overrides are measurement tools rather than new hardware truth until validated.
- The optical input presented to each CGB sensor is the peer emitter state routed by the linked topology; the CGB bus-owned sensor also ORs that input with the machine's own emitter latch to model local self-visibility.
- Frontend readiness indicators must observe each machine's bus-owned `CgbInfraredStatus` instead of duplicating link-topology state, so `IR READY` means both native-CGB sensors are receiver-enabled, warmed, and idle rather than merely having a paired topology.
- Do not route CGB IR through `DMG-04`, `DMG-07`, serial `SB` / `SC`, or frontend transport abstractions. Future netplay or UI light-injection work must attach to an explicit IR seam rather than repurposing link-cable state.
- The current scope is CGB-to-CGB only. Pokémon Pikachu 2, Pocket Sakura, TV remotes, lamps, Chee Chai Alien, HuC1/HuC3-to-CGB IR, and title-specific external protocols require separate device/protocol ownership before they can share this topology.

## Ownership boundary

- `serial` owns `SB`, `SC`, transfer progress, bit shifting, and serial IRQ timing.
- `external_port` owns the per-console attachment identity and the immediate endpoint state presented to `serial`.
- `bus` / CGB infrared state owns `RP`, the emitter latch, read-enable latches, sensor counter, and effective-signal readback.
- `link` owns topology, cable routing, CGB IR peer optical routing, and shared multi-console timing.
- Linked sessions expose public attach / detach operations for session-owned topologies such as `DMG-04` or `DMG-07`; callers should not need to mutate individual member machines' `external_port` attachments just to disconnect or reconnect a session-owned cable or adapter.
- Frontends and harnesses own player-slot UX, windows, audio muting, and host transport.

## Dependencies

- `hardware/SERIAL.md`
- `ARCHITECTURE.md`
- Pan Docs serial and connector sections
- Pan Docs 4-Player Adapter section

## Primary references

- Pan Docs — Serial Data Transfer (Link Cable)
- Pan Docs — External Connectors
- Pan Docs — CGB Infrared Communications Port
- Pan Docs — 4-Player Adapter
- Dan Docs — CGB IR port
- Dan Docs — DMG-07 4-Player Adapter
- Shonumi, "Edge of Emulation: Game Boy 4-Player Adapter"

## Tests

Concrete cases live in link unit/integration tests plus the `gb-test-runner` linked-session manifests and fixtures. Keep this handbook focused on the verification boundaries:

- `DMG-04` topology validation, attach/detach behavior, bidirectional byte exchange, open-line/disconnected input, stale outgoing-byte reuse, simultaneous-internal-clock fallback policy, and at least one retained deterministic chronology fixture
- `DMG-07` physical port identity, `P1` anchor behavior, sparse occupancy, ping acknowledgement / status-byte sampling, `RATE` / `SIZE` capture, transition-marker handling, restart handling, and protocol phase transitions
- `DMG-07` transmission timing, packet length, high-/low-nibble `RATE` effects, one-packet delayed double-buffer behavior, first stale packet, zero-filled absent-port slots, and deterministic 2-, 3-, and 4-console linked sessions
- `CGB IR` pair validation, exact two-participant attach behavior, native-CGB-only `RP` participation, peer emitter sampling, self-emitter visibility, sensor warmup/fade/recovery, warmed-sensor short-pulse visibility, and one retained internal linked-session smoke suite using synthetic CGB ROMs
- shared linked-session stepping on one T-cycle timeline, with frontend or harness tests limited to topology construction, input routing, artifacts, and presentation rather than redefining serial or adapter hardware rules
