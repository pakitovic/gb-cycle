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

Startup must preserve cartridge header SGB capability metadata at `0x0146`, but header parsing remains cartridge-owned. The SGB host owns how header-derived capability policy affects host command acceptance, SGB boot behavior, and diagnostics.

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

SGB screen attributes are host-side 20×18 tile-cell colorization state. They must not be represented as CGB tile attributes, CGB palette indices, hidden GB VRAM state, or frontend-only postprocessing. This separation keeps CGB support and SGB support from contaminating each other.

Borders belong to the SGB host. Static and dynamic borders should update host border tile/palette/attribute state through SGB commands and the 4 KiB transfer path, then compose with the GB LCD image through a frontend-neutral output contract.

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
- Palette and attribute composition tests proving SGB state affects host output without changing DMG PPU state or CGB palette state.
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
