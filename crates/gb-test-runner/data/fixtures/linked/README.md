# Linked-session fixtures

This directory stores repo-owned synthetic fixtures for linked-session validation
in `gb-test-runner`.

The current `dmg04/` set contains two minimal redistributable ROMs plus
retained session-level and participant-level fixtures for a deterministic
two-console `DMG-04` exchange. These fixtures are intended to validate the
linked-session harness and its participant-scoped oracles, not to stand in for
commercial game workflows.

`dmg04/README.md`, `dmg07/README.md`, and `cgb-ir/README.md` record the exact synthetic ROM template and byte program for each committed `.gb` fixture so the binaries remain auditable.

The `stale-*.gb` ROM pair exercises the `DMG-04` stale-byte reuse contract:
the left participant performs two master-clocked transfers without rewriting
`SB`, while the right participant reloads a new byte before the second slave
transfer.

The `double-master-*.gb` ROM pair exercises the current DMG-focused
double-master baseline: both participants select the internal serial clock, so
the link is treated as unsupported and each participant's received `SB` settles
to open-line `0xFF`.

The `open-line-right.gb` ROM leaves the right participant idle while the left
participant performs one internal-clock transfer. That fixture pair validates
the current open-line baseline for a non-participating far end: the active
master still completes and receives `0xFF`.

The `dmg07/` set contains four external-clock slave ROMs for the 4-Player
Adapter. They acknowledge ping, request transmission through P1, place payload
only in the `SIZE` input window, and request a restart with the documented
`0xFF` marker sequence.

The `cgb-ir/` set contains two native-CGB ROMs for the internal `linked-cgb-ir-smoke` suite. One fixture keeps the `RP` emitter latch on; the other enables `RP` sensing, waits for the linked IR signal, and emits serial byte `$B2` as the participant-scoped oracle.
