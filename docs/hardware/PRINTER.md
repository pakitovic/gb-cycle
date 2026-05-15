# PRINTER

## Scope

Own the Game Boy Printer protocol state that lives on the far side of the handheld external port. This includes packet parsing, printer-visible status bits, buffered image data, packet timeout behavior, and typed printed-page artifacts. Do not own `SB` / `SC`, serial bit shifting, or frontend image encoding.

## Layering

- `serial` owns per-console transfer timing, `SB` / `SC`, bit shifting, and the narrow serial-endpoint boundary.
- `external_port` owns whether a printer attachment is present and routes completed serial bytes into the printer protocol state.
- `printer` owns packet-level protocol semantics, printer status, image-buffer management, and typed printed-page output.
- frontends own PNG export, previews, save dialogs, and other presentation policy.

## Responsibilities

- packet framing with magic bytes `$88`, `$33`
- command parsing for `INIT`, `PRINT`, `DATA`, and `STATUS`
- checksum validation
- explicit status-byte composition
- packet timeout reset behavior
- printer image buffer ownership
- typed printed-page artifact generation

## Current v1 baseline

- Commercial compatibility is considered closed for the v1 printer protocol after manual validation with representative printer-enabled software including GB Camera, Pokémon TCG, Pokémon Yellow / Gold / Silver / Crystal, Link's Awakening DX, Super Mario Bros. Deluxe, and long multi-segment Super Mario Bros. Deluxe banner output.
- command support:
  - `0x01` initialize
  - `0x02` print
  - `0x04` data
  - `0x0F` status
- packet checksum validation is implemented
- an empty `DATA` packet must be observed before `PRINT` is accepted
- packet timeout resets an in-progress packet back to the initialized state
- compression flag `0` stores raw `DATA`; compression flag `1` decodes printer RLE for `DATA` packets only
- `DATA` packets are limited to `$280` decoded bytes per segment
- `PRINT` with sheet count `0` is treated as line feed only and does not emit a typed printed page
- printed output is exposed as typed page data, not frontend image files
- current status progression is explicit and deterministic:
  - buffered-but-unprinted data reports `0x08`
  - accepted print work reports `0x06` on the first later status poll
  - completed print work reports `0x04` on the next later status poll

## RLE compression

Compressed `DATA` packet payloads use the Game Boy Printer command stream rather than Game Boy tile encoding. A control byte with bit `7` clear copies the next `(control & $7F) + 1` literal bytes. A control byte with bit `7` set repeats the next byte `(control & $7F) + 2` times. Malformed streams and streams whose decoded segment exceeds `$280` bytes are packet errors and must not mutate the image buffer.

## Typed output contract

The core should expose printed output as typed page/raster data suitable for desktop, CLI, tests, or web hosts. The core must not encode PNG, write files, or assume any one presentation backend.

## Dependencies

- external-port attachment ownership
- serial completed-byte boundary
- shared T-cycle scheduler for packet timeout accounting

## Primary references

- Pan Docs Game Boy Printer section
- Shonumi, "Game Boy Printer" article

## Tests

- packet framing and checksum tests
- detection-sequence tests
- RLE compressed data packet tests
- command payload validation tests
- empty-data-before-print acceptance tests
- packet-timeout reset tests
- typed printed-page raster tests
- integration tests through the machine serial path

## Known deferred work

The remaining items are hardware-fidelity research topics, not blockers for the closed v1 commercial-compatibility baseline:

- more detailed printer-busy timing than the current deterministic status-poll progression
- printer-specific hardware error bits beyond packet/checksum handling
- hardware-verified behavior for broad no-packet idle reset after already-buffered data
- hardware-verified behavior for image-buffer overflow beyond the current deterministic capacity/status model
