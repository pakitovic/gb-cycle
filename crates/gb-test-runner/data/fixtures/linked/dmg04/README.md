# DMG-04 synthetic ROM fixtures

These ROMs are intentionally tiny, redistributable, and fully synthetic. They
exist to exercise linked-session harness contracts, not to emulate a commercial
workflow.

## Common ROM template

Every `.gb` file in this directory is generated from the same minimal template:

- allocate a `32 KiB` ROM image
- fill the whole image with `0xFF`
- write the program bytes below starting at entry point `0x0100`
- set cartridge header bytes:
  - `0x0147 = 0x00` (`ROM ONLY`)
  - `0x0148 = 0x00` (32 KiB ROM size)
  - `0x0149 = 0x00` (no external RAM)

This matches the helper shape used by the linked-session runner tests.

## Fixture programs

### `basic-left.gb`

Purpose:
- send one byte (`0xA5`) as DMG-04 master

Bytes at `0x0100`:

```text
3E A5 E0 01 3E 81 E0 02 C3 08 01
```

Meaning:
- `LD A,$A5`
- `LDH ($01),A`   ; `SB = 0xA5`
- `LD A,$81`
- `LDH ($02),A`   ; `SC = start + internal clock`
- `JP $0108`      ; self-loop after arming transfer

### `basic-right.gb`

Purpose:
- send one byte (`0x3C`) as DMG-04 slave

Bytes at `0x0100`:

```text
3E 3C E0 01 3E 80 E0 02 C3 08 01
```

Meaning:
- same as `basic-left.gb`, but:
  - `SB = 0x3C`
  - `SC = 0x80` (start + external clock)

### `stale-left.gb`

Purpose:
- prove stale-byte reuse on the master side
- perform two transfers without rewriting `SB`

Bytes at `0x0100`:

```text
3E A5 E0 01 3E 81 E0 02 06 FF 05 20 FD 00 00 00 00 00 3E 81 E0 02 C3 16 01
```

Meaning:
- arm first master-clocked transfer with `SB = 0xA5`
- wait in a short countdown loop
- start a second master-clocked transfer with `SC = 0x81`
- do **not** rewrite `SB` before the second transfer

Expected contract:
- emitted serial hex from left participant: `A5A5`

### `stale-right.gb`

Purpose:
- pair with `stale-left.gb`
- rewrite `SB` before the second slave transfer

Bytes at `0x0100`:

```text
3E 3C E0 01 3E 80 E0 02 06 FF 05 20 FD 3E F0 E0 01 3E 80 E0 02 C3 15 01
```

Meaning:
- arm first slave transfer with `SB = 0x3C`
- wait in a short countdown loop
- rewrite `SB = 0xF0`
- arm second slave transfer with `SC = 0x80`

Expected contract:
- emitted serial hex from right participant: `3CF0`

### `double-master-left.gb`

Purpose:
- exercise current DMG-focused unsupported double-master baseline

Bytes at `0x0100`:

```text
3E A5 E0 01 3E 81 E0 02 C3 08 01
```

Meaning:
- same program shape as `basic-left.gb`

### `double-master-right.gb`

Purpose:
- pair with `double-master-left.gb`, but also select internal clock

Bytes at `0x0100`:

```text
3E 3C E0 01 3E 81 E0 02 C3 08 01
```

Meaning:
- same shape as `basic-right.gb`, except `SC = 0x81`

Expected contract:
- both participants receive open-line `0xFF` under the current baseline

### `open-line-right.gb`

Purpose:
- leave the far end idle while the left participant performs one transfer

Bytes at `0x0100`:

```text
C3 00 01
```

Meaning:
- `JP $0100`
- infinite idle loop; never writes `SB` or `SC`

Expected contract:
- active master still completes and receives `0xFF`

## Audit rule

`crates/gb-test-runner/tests/linked_fixture_roms.rs` reconstructs these ROMs
from the byte sequences above and verifies that the committed `.gb` files match
exactly. If any ROM is intentionally changed, update both this README and that
test in the same change.
