# PRINTER

## Scope

Own the Game Boy Printer protocol state that lives on the far side of the
handheld external port. This includes packet parsing, printer-visible status
bits, buffered image data, packet timeout behavior, and typed printed-page
artifacts. Do not own `SB` / `SC`, serial bit shifting, or frontend image
encoding.

## Layering

- `serial` owns per-console transfer timing, `SB` / `SC`, bit shifting, and the
  narrow serial-endpoint boundary.
- `external_port` owns whether a printer attachment is present and routes
  completed serial bytes into the printer protocol state.
- `printer` owns packet-level protocol semantics, printer status, image-buffer
  management, and typed printed-page output.
- frontends own PNG export, previews, save dialogs, and other presentation
  policy.

## Responsibilities

- packet framing with magic bytes `$88`, `$33`
- command parsing for `INIT`, `PRINT`, `DATA`, and `STATUS`
- checksum validation
- explicit status-byte composition
- packet timeout reset behavior
- printer image buffer ownership
- typed printed-page artifact generation

## Current v1 baseline

- command support:
  - `0x01` initialize
  - `0x02` print
  - `0x04` data
  - `0x0F` status
- packet checksum validation is implemented
- an empty `DATA` packet must be observed before `PRINT` is accepted
- packet timeout resets the printer back to its initialized state
- compression flag `1` is currently rejected as packet error; compressed data is
  not implemented yet
- printed output is exposed as typed page data, not frontend image files
- current status progression is explicit and deterministic:
  - buffered-but-unprinted data reports `0x08`
  - accepted print work reports `0x06` on the first later status poll
  - completed print work reports `0x04` on the next later status poll

## Typed output contract

The core should expose printed output as typed page/raster data suitable for
desktop, CLI, tests, or web hosts. The core must not encode PNG, write files,
or assume any one presentation backend.

## Dependencies

- external-port attachment ownership
- serial completed-byte boundary
- shared T-cycle scheduler for packet timeout accounting

## Primary references

- Pan Docs Game Boy Printer section

## Tests

- packet framing and checksum tests
- detection-sequence tests
- empty-data-before-print acceptance tests
- packet-timeout reset tests
- typed printed-page raster tests
- integration tests through the machine serial path

## Known deferred work

- compressed data packets
- more detailed printer-busy timing than the current deterministic status-poll
  progression
- printer-specific hardware error bits beyond packet/checksum handling
