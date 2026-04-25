# DMG-07 synthetic ROM fixtures

These ROMs are tiny redistributable fixtures for the linked-session runner. They exercise the DMG-07 adapter protocol with external-clock slave transfers; they are not commercial workflows.

## Common ROM template

Every `.gb` file in this directory is generated from the same `32 KiB` ROM template used by the DMG-04 fixtures:

- fill the image with `0xFF`
- write the program below at `0x0100`
- set header bytes `0x0147 = 0x00`, `0x0148 = 0x00`, `0x0149 = 0x00`

## Program prefix

Bytes at `0x0100` before the table:

```text
21 14 01 7E E0 01 3E 80 E0 02 F0 02 CB 7F 20 FA 23 C3 03 01
```

Meaning:

- `LD HL,$0114` points at the response table.
- Load one response byte from `(HL)` into `SB`.
- Arm serial as external-clock slave with `SC = 0x80`.
- Poll `SC.7` until the adapter completes the byte.
- Increment `HL` and repeat.

## Response tables

`p1-basic.gb` table:

```text
88 88 00 01 AA AA AA 00 00 00 00 00 A1 00 00 00 A2 00 00 00 FF FF FF 00 88 88 00 01
```

`p2-basic.gb` table:

```text
88 88 00 01 00 00 00 00 00 00 00 00 B1 00 00 00 B2 00 00 00 00 00 00 00 88 88 00 01
```

`p3-basic.gb` table:

```text
88 88 00 01 00 00 00 00 00 00 00 00 C1 00 00 00 C2 00 00 00 00 00 00 00 88 88 00 01
```

`p4-basic.gb` table:

```text
88 88 00 01 00 00 00 00 00 00 00 00 D1 00 00 00 D2 00 00 00 00 00 00 00 88 88 00 01
```

The first four bytes acknowledge ping and configure `RATE = 0`, `SIZE = 1`. P1 then sends the compatibility transition sequence `AA AA AA 00`; later it sends `FF FF FF 00` to request restart. Payload bytes are placed only in each packet's `SIZE` input window; filler bytes remain `0x00`.

## Audit rule

`crates/gb-test-runner/tests/linked_fixture_roms.rs` reconstructs these ROMs from the byte sequences above and verifies that the committed `.gb` files match exactly. If any ROM is intentionally changed, update both this README and that test in the same change.
