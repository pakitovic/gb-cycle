# Linked-session fixtures

This directory stores repo-owned synthetic fixtures for linked-session validation
in `gb-test-runner`.

The current `dmg04/` set contains two minimal redistributable ROMs plus
retained session-level and participant-level fixtures for a deterministic
two-console `DMG-04` exchange. These fixtures are intended to validate the
linked-session harness and its participant-scoped oracles, not to stand in for
commercial game workflows.

`dmg04/README.md` records the exact synthetic ROM template and the byte program
for each committed `.gb` fixture so the binaries remain auditable.

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
