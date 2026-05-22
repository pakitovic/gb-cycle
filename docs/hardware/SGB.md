# SGB

## Scope

Own future Super Game Boy and Super Game Boy 2 behavior as a host-shell hardware model around the shared GB core. This document defines SGB/SGB2 boundaries, command ownership, startup profiles, timing profile expectations, host video/audio composition, multiplayer behavior, SGB2 link behavior, and the future SNES-side execution seam.

## Hardware model

SGB is a DMG-compatible Game Boy core embedded behind an ICD2/SNES/SFC host shell. The GB core still owns CPU, PPU, APU, timer, DMA, bus, cartridge, serial transfer semantics, interrupts, and the T-cycle scheduler; the SGB host owns JOYP packet interpretation, SGB command state, host-side palette and attribute maps, borders, multiplayer controller multiplexing, special host audio, SGB/SGB2 profile timing, and SNES-side data/execution behavior.

Do not treat SGB as CGB mode and do not implement a second DMG emulator for SGB. SGB behavior should enter through the `HostPlatform` axis and derived capabilities while `OperatingMode` remains a DMG-compatible software contract. Use `HostPlatform::Sgb` for the original Super Game Boy whenever possible and reserve `HostPlatform::Sgb2` for Super Game Boy 2.

The host implementation must remain pluggable. Early implementation may use a deterministic HLE host for command effects, but command/data/audio interfaces must leave room for later real or equivalent SNES-side execution, especially for `DATA_SND`, `DATA_TRN`, `JUMP`, S-APU-related behavior, and Space Invaders-style host code.

## Public profiles

| UI label | Machine profile | Host platform | GB core contract | Revision | Startup modes | RealBoot asset | Video standard | Host capabilities |
|---|---|---|---|---|---|---|---|---|
| `SUPER GB` | `SGB` | `Sgb` | DMG-compatible | `SGB-CPU 01` | `skip-boot`, `real-boot` | `sgb_boot.bin` | `PAL` or `NTSC` | SGB command host, palettes, borders, multiplayer, host audio, SNES-side command/data path; no physical Game Link port. |
| `SUPER GB 2` | `SGB2` | `Sgb2` | DMG-compatible | `CPU SGB2` | `skip-boot`, `real-boot` | `sgb2_boot.bin` | `NTSC` | SGB command host, palettes, borders, multiplayer, host audio, corrected clock versus SGB, physical Game Link support. |

`MODEL: SGB` and `MODEL: SGB2` are user-facing machine profiles that resolve into explicit model axes. They must not become independent duplicated cores. If future code needs an internal profile descriptor, it should derive from the selected host platform, revision, startup mode, and video standard rather than becoming a loose collection of flags.

## Boot and startup

SGB/SGB2 must support `SkipBoot` and `RealBoot` as distinct startup paths. `RealBoot` executes the revision-derived boot ROM on the shared CPU/bus/scheduler path with the boot ROM mapped until the real handoff; `SkipBoot` synthesizes a coherent post-boot handoff state and does not require boot ROM bytes.

For `SUPER GB`, `RealBoot` selects `sgb_boot.bin` for revision `SGB-CPU 01`. For `SUPER GB 2`, `RealBoot` selects `sgb2_boot.bin` for revision `CPU SGB2`. A documented private asset root example is `$HOME/emu/roms/bootrom`, containing `sgb_boot.bin` and/or `sgb2_boot.bin`; validation should follow the existing boot-ROM root policy rather than embedding private paths in code.

Startup must preserve cartridge header SGB capability metadata at `0x0146`, but header parsing remains cartridge-owned. The SGB host owns how header-derived capability policy affects host command acceptance, SGB boot behavior, and diagnostics. The Slice 1 HLE host accepts command packets only when the loaded cartridge header has SGB flag `$0146 == $03` and old licensee `$014B == $33`; active SGB/SGB2 hosts otherwise keep packet transport observable but reject complete packets before command side effects.

## Responsibilities

- Host-shell profile selection for SGB NTSC, SGB PAL, and SGB2 NTSC.
- JOYP/P1 SGB packet framing, accumulation, decode, validation, tracing, and command dispatch.
- SGB palette RAM, active palette selection, attribute maps, mask state, and host-side final colorization.
- Border tile/palette/attribute state and composition around the GB LCD image.
- Shared 4 KiB VRAM-transfer capture path for `_TRN` commands.
- `MLT_REQ` controller multiplexing for one, two, and four players.
- SGB special-audio command state and host-audio export/mixing boundaries.
- SGB2 physical Game Link availability and routing through existing link topology boundaries.
- SNES/SFC-side data transfer and eventual 16-bit execution hooks.
- Save-state coverage for every live host state introduced by an implementation slice.

## Registers / MMIO

SGB does not add ordinary GB MMIO registers in the CGB sense. The primary GB-visible transport is the JOYP/P1 register path, where software writes packet bits through bits 4 and 5 using the SGB command protocol while still sharing the ordinary joypad register address.

The GB joypad subsystem should expose a narrow SGB host transport boundary rather than allowing the SGB host to poll arbitrary bus state. Ordinary P1 row selection and button reads remain joypad-owned; SGB packet collection and multiplayer host responses are SGB-host behavior layered at that boundary.

Physical serial link behavior remains serial/link-owned. Original SGB has no Game Link port, while SGB2 exposes physical Game Link support that must route through the existing link topology rather than through the SGB command-packet path.

## JOYP packet transport

SGB command transport uses active-low JOYP/P1 bits 4 and 5 as a host-side serial protocol. The host treats both lines high (`$30`) as idle, both lines low (`$00`) as the packet reset/start pulse, P14 low with P15 high (`$20`) as data bit `0`, and P14 high with P15 low (`$10`) as data bit `1`. Data bits are accumulated only from an idle-to-data transition so repeated writes without the idle separator do not create extra bits.

Each packet carries 16 bytes / 128 data bits, least-significant bit first within each byte, followed by a stop bit `0`. The first byte encodes command ID in bits 3..7 and packet count in bits 0..2; the current host accepts packet counts `1..=7` and records packet-count `0` or impossible framing as invalid before any command-specific mutation. Multi-packet commands keep the active command ID and expected packet count in host state, but Slice 1 still keeps all command side effects inert.

Packet transport state is part of `SgbHostSaveState`: last line state, active transfer flag, buffered bit/byte counts, current 16-byte buffer, pulse counters, last packet trace, active command ID, expected/received packet counts, accepted command count, rejected packet count, and invalid packet count. Save/load must resume a partially buffered packet exactly instead of re-reading P1 or replaying writes.

## Command ownership matrix

| Command family | Examples | Owning state | Implementation notes |
|---|---|---|---|
| Packet/control baseline | packet header, packet count, invalid packet handling | SGB host packet decoder | Decode before side effects; trace raw packets and command IDs. |
| Palettes | `PAL01`, `PAL23`, `PAL03`, `PAL12`, `PAL_SET`, `PAL_TRN`, `PAL_PRI` | SGB host palette state | Map DMG pixel shade indices through SGB palettes; do not use CGB palette RAM. |
| Attributes | `ATTR_BLK`, `ATTR_LIN`, `ATTR_DIV`, `ATTR_CHR`, `ATTR_TRN`, `ATTR_SET` | SGB host 20×18 attribute map and loaded attribute buffers | Keep separate from GB PPU tile metadata and CGB attributes. |
| Borders and masks | `MASK_EN`, `CHR_TRN`, `PCT_TRN` | SGB host border tiles, tilemap/attributes, palettes, mask state | Consume the shared 4 KiB transfer path and compose outside the GB LCD image. |
| Multiplayer | `MLT_REQ` | SGB host controller mux | Support one/two/four-player modes and P1 player-ID cycling; separate from Game Link. |
| Host audio | `SOUND`, `SOU_TRN` | SGB host audio event/state | Keep ordinary GB APU separate; allow deterministic HLE first and later S-APU-capable backend. |
| System/data/execution | `DATA_SND`, `DATA_TRN`, `JUMP` | Pluggable SNES/SFC host backend | Final-stage behavior; avoid game-specific shortcuts and leave room for 65C816 execution. |

## Video and color composition

The GB PPU still produces the 160×144 DMG LCD image with DMG pixel/shade information and sprite/background composition according to the GB core. The SGB host then maps those DMG shade indices through SGB palette and attribute state and places the resulting image inside the host SNES/SFC output with optional border graphics.

Slice 2 defines the first visible host-color path as a 160×144 LCD RGB555 composition result. This output is not CGB palette RAM and does not change the GB PPU framebuffer; it maps the already-produced DMG panel shade index through the SGB host's current screen palette 0 until later attribute commands select different palettes per 20×18 cell.

The SGB host stores four screen palettes, each with four RGB555 colors in the same bit layout used by SGB/SNES command payloads: bit 15 ignored, bits 0..4 red, bits 5..9 green, bits 10..14 blue, little-endian in command packets. Initial SGB/SGB2 host state uses a deterministic DMG grayscale palette for all four screen palettes so composition is defined before any game command or user palette selection is implemented.

Slice 2 implements the direct one-packet palette commands `PAL01`, `PAL23`, `PAL03`, and `PAL12`. Each command updates two physical screen palettes: one shared color 0 for the pair, three colors for the first palette, and three colors for the second palette. `PAL_SET`, `PAL_TRN`, and `PAL_PRI` remain later-slice commands because they depend on system palette transfer/selection and priority behavior outside the base direct-palette path.

Slice 3 adds the shared 4 KiB `_TRN` capture seam. `CHR_TRN` and `PCT_TRN` are still normal one-packet SGB commands, but completing the packet only schedules a host-side transfer request; the machine captures the first 4 KiB of raw GB VRAM at the next PPU frame-start boundary and then lets the SGB host decode the payload. This keeps the command path faithful to the video-transfer model described by Pan Docs while leaving room for a later multi-frame timing refinement; it is not a CPU VRAM read, not CGB VRAM behavior, and not frontend-only postprocessing.

`CHR_TRN` decodes the command destination byte into tile block `$00-$7F` or `$80-$FF` and BG/OBJ tile type metadata, then writes the captured 4 KiB payload into SGB border tile memory. The BG/OBJ bit is retained as command metadata; the initial HLE border backend writes the same tile-memory range because Pan Docs notes that the bit appears not to change the address used by SGB software. Each SGB border tile is stored as 32 bytes of 4bpp SNES tile data: planes 0/1 interleaved by row, followed by planes 2/3 interleaved by row.

`PCT_TRN` decodes the captured payload as border screen data: bytes `$000-$6FF` hold the 32×28 visible SNES BG map, bytes `$700-$73F` hold the extra 29th row that can be visible as a flicker line on real output, and bytes `$800-$85F` hold three 16-color little-endian RGB555 palettes corresponding to SNES BG palettes 4, 5, and 6. The host stores 29 rows so the extra-row data survives save/load even though the current 256×224 composition surface displays only the 28 visible rows.

The full SGB host-frame output contract is `SgbHost::compose_frame_rgb555` / `Machine::sgb_framebuffer_rgb555`: a 256×224 RGB555 image with the 160×144 GB LCD window at `(48, 40)`. Border pixels outside the GB window always come from the border tilemap; inside the GB window, border color index 0 lets the composed SGB LCD image show through, while non-zero border pixels cover the GB window. This models the SGB border as host state and preserves the GB PPU framebuffer as the source of the LCD image.

`MASK_EN` is host-video state. `Cancel` displays the live SGB LCD image, `Freeze` snapshots the current host LCD RGB555 image and continues using it, `BlankBlack` displays RGB555 black, and `BlankColor0` displays the current SGB screen palette color 0. These modes affect the host LCD composition boundary and do not alter GB VRAM, GB PPU timing, DMG palette registers, or CGB palette hardware.

Slice 4 makes SGB screen attributes active host-side 20×18 tile-cell colorization state. `ATTR_BLK`, `ATTR_LIN`, `ATTR_DIV`, and `ATTR_CHR` mutate an explicit SGB attribute map whose cells select one of the four SGB screen palettes for the already-rendered DMG LCD shade; they must not be represented as CGB tile attributes, CGB palette indices, hidden GB VRAM state, or frontend-only postprocessing. This separation keeps CGB support and SGB support from contaminating each other.

`PAL_TRN` and `ATTR_TRN` share the Slice 3 `_TRN` seam. `PAL_TRN` captures 4 KiB and decodes it as 512 system palettes of four little-endian RGB555 colors; this memory is not directly visible until `PAL_SET` copies selected logical palettes into the four physical SGB screen palettes. `ATTR_TRN` captures 4 KiB and stores the first 4050 bytes as 45 Attribute Files, each containing 90 packed bytes for a 20×18 two-bit palette-index map.

`PAL_SET` copies four little-endian system-palette IDs into physical palettes 0-3, records `PAL_PRI` priority state, and optionally applies an Attribute File or cancels a current `MASK_EN` state. `ATTR_SET` copies one Attribute File into the active 20×18 map and can also cancel `MASK_EN`. Invalid ATF IDs above `$2C` are retained as deterministic host diagnostics and do not mutate the active attribute map.

The SGB LCD composition path now selects a palette per 8×8 LCD cell before mapping the DMG shade to RGB555. `Freeze` snapshots the already colorized host LCD image, including the active attribute map; later palette or attribute changes do not alter the frozen image until mask cancellation. This keeps advanced screen coloring as a host-composition layer over the DMG framebuffer and leaves GB PPU fetch/render timing unchanged.

Borders belong to the SGB host. Static and dynamic borders update host border tile/palette/attribute state through SGB commands and the 4 KiB transfer path, then compose with the GB LCD image through the frontend-neutral 256×224 host-frame contract.

## Multiplayer

`MLT_REQ` enables SGB controller multiplexing for one, two, or four players. The SGB host owns player count, selected player, player-ID cycling, and routing of host input slots to GB-visible joypad reads. Frontends and test tooling should expose player slots without binding the GB core to any UI event model.

SGB multiplayer is not a DMG-07 four-player adapter and not a Game Link connection. SGB2 Game Link support is a physical serial/link feature and must remain separate from `MLT_REQ` controller multiplexing.

## Audio

The GB APU remains the shared core's ordinary audio generator. SGB special audio is host-side behavior driven by `SOUND` and `SOU_TRN`, exported or mixed through an SGB host-audio boundary.

A deterministic HLE host-audio backend is acceptable for initial command/state support, but the API must leave room for a later SNES/S-APU-capable backend. Do not hardcode Donkey Kong (GB) special audio behavior as a title-specific path; use it only as a manual compatibility example.

## Timing / accuracy requirements

Use T-cycles as the GB core timing unit as elsewhere in the project, but keep SGB host profile timing explicit. `SGB NTSC`, `SGB PAL`, and `SGB2 NTSC` must be represented as profile facts so video standard, corrected SGB2 clock behavior, and host presentation timing are not inferred from arbitrary names or frontend settings.

SGB2 corrected clock behavior belongs to the SGB2 profile. Original SGB may have PAL/NTSC host variants. SGB2 is NTSC in the planned public profile unless future evidence requires a broader matrix.

Packet decode, command side effects, transfer capture, controller multiplexing, host audio events, and future SNES execution all require deterministic save/load continuation. Do not batch or approximate state in ways that prevent reproducing first divergence after restore.

## SGB2 Game Link

Original SGB has no physical Game Link port. SGB2 has Game Link support and should route through the existing link topology and serial boundaries, not through the SGB command-packet layer.

SGB2 link availability should be a profile/capability fact. Tests should distinguish SGB's no-link behavior from SGB2 link attachment and should prove that SGB2 link routing does not reimplement serial transfer semantics inside the SGB host.

## SNES-side execution

`DATA_SND`, `DATA_TRN`, and `JUMP` require a host backend that can own SNES/SFC-side memory, data-transfer destinations, and execution state. A command-only HLE host is acceptable for earlier slices, but the final execution slice must be able to model 16-bit host-side execution sufficiently for software that uploads and jumps to SNES code.

Space Invaders should be treated as a manual compatibility example for this capability, not as a hardcoded special case.

## Dependencies

- `core/MODEL-AXES.md` for `HostPlatform`, operating-mode, and capability guidance.
- `hardware/BOOT-ROM.md` for startup mode and boot-ROM asset policy.
- `hardware/JOYPAD.md` for P1/JOYP ownership and button-read semantics.
- `hardware/PPU.md` for the DMG LCD image source consumed by SGB host composition.
- `hardware/APU.md` for the GB APU boundary that SGB host audio must not replace.
- `hardware/SERIAL.md` and `hardware/LINK.md` for SGB2 Game Link routing.
- `docs/roadmap/11-sgb.md` for implementation sequencing.

## Primary references

- Pan Docs SGB Functions: https://gbdev.io/pandocs/SGB_Functions.html
- Pan Docs SGB Command Packet: https://gbdev.io/pandocs/SGB_Command_Packet.html
- Pan Docs SGB Command Summary: https://gbdev.io/pandocs/SGB_Command_Summary.html
- Pan Docs SGB VRAM Transfers: https://gbdev.io/pandocs/SGB_VRAM_Transfer.html
- Pan Docs SGB Palettes: https://gbdev.io/pandocs/SGB_Command_Palettes.html
- Pan Docs SGB Borders: https://gbdev.io/pandocs/SGB_Command_Border.html
- Pan Docs SGB Multiplayer: https://gbdev.io/pandocs/SGB_Command_Multiplayer.html
- Pan Docs SGB Sound: https://gbdev.io/pandocs/SGB_Command_Sound.html
- Pan Docs SGB System Commands: https://gbdev.io/pandocs/SGB_Command_System.html
- Gekkio research and hardware references when SGB/SGB2 startup, clocks, boot ROMs, or ICD2 behavior need hardware confirmation.

## Open-source emulator references

- SameBoy for practical SGB/SGB2 architecture, command behavior, and differential comparison after primary references.
- bsnes/higan-class SNES references for later host-side 65C816/SNES/S-APU behavior when Slice 8 begins.
- GBE+ as an accessory/peripheral-oriented cross-check after primary references, especially for obscure SGB-adjacent behavior.

Open-source emulator code is a comparison aid, not hardware truth. If references disagree, prefer real hardware documentation/research, then model clarity and deterministic tests.

## Tests

- Packet-decode unit tests for JOYP bit framing, packet counts, command IDs, malformed packets, reset behavior, and partial-packet save/load.
- SameSuite SGB `command_mlt_req.gb` and `command_mlt_req_1_incrementing.gb` run as informational `console = "sgb"` rows after Slice 1 so packet/startup traces are visible early; they become blocking multiplayer evidence only when Slice 5 implements `MLT_REQ`.
- Palette composition tests proving direct `PALxx` commands affect host RGB555 LCD output without changing DMG PPU state or CGB palette state; attribute composition remains Slice 4.
- Border transfer/composition tests for static border load, repeated updates, mask behavior, and save/load continuation.
- `MLT_REQ` tests for one/two/four-player modes, player selection, P1 cycling, and frontend/test-runner input slots.
- SGB/SGB2 profile tests for PAL/NTSC validity, corrected SGB2 clock profile, original SGB no-link behavior, and SGB2 Game Link availability.
- Host-audio command tests for deterministic event/state capture without replacing the GB APU.
- SNES-host seam tests for `DATA_SND`, `DATA_TRN`, `JUMP`, backend state persistence, and deterministic continuation.

Commercial titles are manual compatibility examples only unless a future private-suite policy explicitly changes that. Do not add them to CI or public gates.

## Implementation notes for this repo

- Keep SGB concerns out of unrelated DMG/CGB subsystems unless the subsystem owns the boundary being touched.
- The intended default shape is "shared GB core plus SGB host shell": the shared core owns CPU / PPU / APU / DMA / timer / bus truth, while the SGB layer owns packet interpretation, borders, colorization, multiplayer host behavior, host audio, and SNES-side coordination.
- SGB should reuse the DMG-family shared path through explicit axes and capabilities, not a dedicated "SGB core".
- SGB palette/attribute/border state must be explicit host state and must not piggyback on CGB palette or CGB tile-attribute internals.
- Every slice that adds live host state must update typed save states before the slice is considered closed.
- The Slice 0 baseline has an explicit inert `SgbHost` block in machine state, snapshots, and save states. It owns profile descriptors, deterministic-HLE backend identity, video/multiplayer/audio/SNES-side placeholder state, and SGB2 capability facts. Slice 1 extends that block with startup mode, real-boot asset intent, header-derived command acceptance, JOYP packet decode state, command packet counters, and packet traces observed from `FF00` writes. Slice 2 adds persistent direct-palette state, a seven-packet command buffer for future multi-packet commands, and `SgbHost::compose_lcd_rgb555` / `Machine::sgb_lcd_framebuffer_rgb555` as the frontend-neutral base LCD color output. Slice 3 adds pending `_TRN` transfer state, the retained last 4 KiB transfer payload, border tile data, border tilemap/palette state, `MASK_EN` mask/freeze state, and `SgbHost::compose_frame_rgb555` / `Machine::sgb_framebuffer_rgb555` for the 256×224 host frame. Slice 4 adds active 20×18 screen attributes, packed ATF memory, 512 system palettes, indirect palette/attribute commands, `PAL_PRI`, and attribute-aware LCD/freeze composition; because this changes the typed whole-machine save-state payload again, the durable machine save-state format version is bumped.

## Known pitfalls

- Naming the original Super Game Boy `SGB1` in user-facing docs or APIs when `SGB` / `Sgb` is sufficient.
- Treating SGB as CGB mode or reusing CGB palette/tile-attribute state for SGB colorization.
- Letting frontend-only presentation own border or palette behavior that should be deterministic core/host state.
- Mixing `MLT_REQ` controller multiplexing with DMG-07 or SGB2 Game Link semantics.
- Building an HLE-only SGB host API that cannot later support `DATA_SND`, `DATA_TRN`, `JUMP`, or SNES-side execution.
- Hardcoding commercial game behavior instead of implementing command/state semantics.

## Open questions

- Which SGB/SGB2 boot-ROM hashes and exact boot mapping windows should become strict once private boot-ROM validation is added.
- Which SNES/SFC host backend is preferred for Slice 8: a minimal in-repo execution model, a narrow pluggable backend, or an integration with a broader SNES core.
- What frontend output surface should represent the combined SGB border plus GB LCD image without disrupting existing handheld framebuffer APIs.
