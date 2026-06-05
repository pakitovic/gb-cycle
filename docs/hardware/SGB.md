# SGB

## Scope

Own Super Game Boy and Super Game Boy 2 behavior as a host-shell hardware model around the shared GB core. This document defines SGB/SGB2 boundaries, command ownership, startup profiles, timing profile expectations, host video composition, multiplayer behavior, SGB2 link behavior, and diagnostic host-backend request seams.

## Hardware model

SGB is a DMG-compatible Game Boy core embedded behind an ICD2/SNES/SFC host shell. The GB core still owns CPU, PPU, APU, timer, DMA, bus, cartridge, serial transfer semantics, interrupts, and the T-cycle scheduler; the SGB host owns JOYP packet interpretation, SGB command state, host-side palette and attribute maps, borders, multiplayer controller multiplexing, SGB/SGB2 profile timing, and diagnostic request seams for host audio and SNES-side data/execution commands.

Do not treat SGB as CGB mode and do not implement a second DMG emulator for SGB. SGB behavior should enter through the `HostPlatform` axis and derived capabilities while `OperatingMode` remains a DMG-compatible software contract. Use `HostPlatform::Sgb` for the original Super Game Boy whenever possible and reserve `HostPlatform::Sgb2` for Super Game Boy 2.

The host implementation keeps pluggable command/data/audio seams, but the current `gb-cycle` architecture does not implement a real SNES/SFC host. `SOUND`, `SOU_TRN`, `DATA_SND`, `DATA_TRN`, `JUMP`, S-APU/S-DSP behavior, and Space Invaders-style host code are diagnostic request/state boundaries unless a future project explicitly incorporates a complete SNES/SFC emulator.

## Public profiles

| UI label | Machine profile | Host platform | GB core contract | Revision | Startup modes | RealBoot asset | Video standard | Host capabilities |
|---|---|---|---|---|---|---|---|---|
| `SUPER GB` | `SGB` | `Sgb` | DMG-compatible | `SGB-CPU 01` | `skip-boot`, `custom-boot`, `real-boot` | `sgb_boot.bin` | `PAL` or `NTSC` | SGB command host, palettes, borders, multiplayer, diagnostic host-audio and SNES-side request seams; no physical Game Link port. |
| `SUPER GB2` | `SGB2` | `Sgb2` | DMG-compatible | `CPU SGB2` | `skip-boot`, `custom-boot`, `real-boot` | `sgb2_boot.bin` | `NTSC` | SGB command host, palettes, borders, multiplayer, diagnostic host-audio and SNES-side request seams, corrected clock versus SGB, physical Game Link support. |

`MODEL: SGB` and `MODEL: SGB2` are user-facing machine profiles that resolve into explicit model axes. They must not become independent duplicated cores. The internal profile descriptor is `SgbHostProfile`: `SgbNtsc` and `SgbPal` require `HostPlatform::Sgb`, while `Sgb2Ntsc` requires `HostPlatform::Sgb2`; impossible combinations such as PAL SGB2 are rejected by model-axis coherence and save-state metadata validation instead of being normalized silently.

`gb-desktop` and `gb-cli` expose `SGB` and `SGB2` through the same public model/profile selector as handheld models. The model selector maps `SGB` through an explicit video-standard axis, defaulting to `SgbHostProfile::SgbNtsc` and allowing `SgbHostProfile::SgbPal` via `--sgb-standard pal` or `CONFIG -> SYSTEM -> VIDEO PAL`; `SGB2` always maps to `SgbHostProfile::Sgb2Ntsc`, rejects an explicit CLI SGB standard, and shows a disabled `VIDEO NTSC` UI item instead of introducing an impossible PAL SGB2 profile or `SGB1` naming.

## Current implementation level

The current public SGB/SGB2 implementation covers the Phase 11 slices 0-6 milestone plus post-slice hardening: architecture and save-state shape, profile-aware `SkipBoot`/`CustomBoot` direct-start registers, `RealBoot` asset selection, JOYP packet transport, SGB-header unlock policy, deterministic packet busy/suppression gating, base palette commands, BIOS title/default palette seeding for DMG-only titles, `_TRN` transfer scheduling and five-frame capture state, static/dynamic borders, `MASK_EN`, advanced screen coloring, transferred palette/attribute files, `PAL_PRI`, `MLT_REQ` multiplayer, `ATRC_EN`, `TEST_EN`, `ICON_EN`, `OBJ_TRN` state, SGB NTSC/PAL and SGB2 NTSC profile timing, desktop/CLI model exposure, SGB border presentation toggles, and SGB2 physical Game Link routing.

Slices 7, 8, and 9 are not planned for the current `gb-cycle` architecture. Experiments for general SGB special audio through `SOUND`/`SOU_TRN`, SNES-side data transfer and 16-bit execution through `DATA_SND`/`DATA_TRN`/`JUMP`, and the SNES/SFC-side startup shell were inconclusive and showed that strict compatibility requires a complete SNES/SFC emulator with 65C816 execution, S-APU/S-DSP audio, host firmware behavior, and presentation state. Current `RealBoot` executes the GB-side 256-byte `sgb_boot.bin` / `sgb2_boot.bin` assets and implemented host command effects, but it does not execute or fake the real SNES/SFC firmware startup presentation or provide real SGB special audio.

## Boot and startup

SGB/SGB2 must support `SkipBoot`, `CustomBoot`, and `RealBoot` as distinct startup paths. `RealBoot` executes the SGB-profile-derived boot ROM on the shared CPU/bus/scheduler path with the boot ROM mapped until the real handoff; `SkipBoot` and `CustomBoot` synthesize coherent profile-aware post-boot handoff state and do not require boot ROM bytes.

For `SUPER GB`, `RealBoot` selects `sgb_boot.bin` for revision label `SGB-CPU 01`. For `SUPER GB2`, `RealBoot` selects `sgb2_boot.bin` for revision label `CPU SGB2`. A documented private asset root example is `$HOME/emu/roms/bootrom`, containing `sgb_boot.bin` and/or `sgb2_boot.bin`; validation follows the existing boot-ROM root policy rather than embedding private paths in code. These assets are selected by `SgbHostProfile`, not by forking the GB `HardwareRevision` axis or by aliasing to `dmg_boot.bin`.

`SkipBoot` and `CustomBoot` direct-start CPU state is also SGB-profile-aware. DMG-family handheld direct start still exposes the existing DMG fingerprint such as `C=$13`, while original SGB direct start exposes `A=$01` and `C=$14`, and SGB2 direct start exposes `A=$FF` and `C=$14`. This state comes from `SgbHostProfile`; it must not be modeled by enabling CGB state or by applying frontend-only register patches.

Strict asset verification treats both SGB boot ROMs as `256`-byte low-window images: `sgb_boot.bin` has SHA-256 `0e4ddff32fc9d1eeaae812a157dd246459b00c9e14f2f61751f661f32361e360`, and `sgb2_boot.bin` has SHA-256 `fd243c4fb27008986316ce3df29e9cfbcdc0cd52704970555a8bb76edbec3988`.

Current SGB/SGB2 `RealBoot` executes only this 256-byte GB-side boot ROM on the shared DMG-compatible CPU/bus path. It does not execute the SNES/SFC-side SGB firmware or seed the host's built-in startup border/animation state, so the deterministic host starts with blank border VRAM/CGRAM until the boot ROM or cartridge command stream mutates implemented host state. The real-hardware Super Game Boy logo animation and built-in "Game Boy screen" border are SNES-side host-shell behavior that is not planned for the current architecture; they are not contained in `sgb_boot.bin` / `sgb2_boot.bin` themselves, and `.sfc` firmware files are not active assets in the current implementation.

Startup must preserve cartridge header SGB capability metadata at `0x0146`, but header parsing remains cartridge-owned. The SGB host owns how header-derived capability policy affects host command acceptance, SGB boot behavior, and diagnostics. The Slice 1 HLE host accepts command packets only when the loaded cartridge header has SGB flag `$0146 == $03` and old licensee `$014B == $33`; active SGB/SGB2 hosts otherwise keep packet transport observable but reject complete packets before command side effects.

At SGB/SGB2 `RealBoot` handoff, the real `FF50` unmap edge is also an SGB-host packet boundary: incomplete boot-ROM packet accumulation and active command state are cleared while accepted boot-packet counters and host video state remain intact. This prevents SGB boot-private multi-packet traffic from absorbing the first cartridge-side commands, such as the `MLT_REQ` probe used by Donkey Kong (GB), after cartridge code starts at `0x0100`.

## Responsibilities

- Host-shell profile selection for SGB NTSC, SGB PAL, and SGB2 NTSC.
- JOYP/P1 SGB packet framing, accumulation, decode, validation, tracing, and command dispatch.
- SGB palette RAM, active palette selection, attribute maps, mask state, and host-side final colorization.
- Border tile/palette/attribute state and composition around the GB LCD image.
- Shared 4 KiB VRAM-transfer state for `_TRN` commands, including pending phase, partial payload, final payload, and completion counters.
- `MLT_REQ` controller multiplexing for one, two, and four players.
- SGB special-audio command state and host-audio export/mixing boundaries.
- SGB system/menu command state for `ATRC_EN`, `TEST_EN`, `ICON_EN`, and explicit `OBJ_TRN` host OBJ transfer capability state.
- Deterministic packet busy/suppression windows expressed in host frames/T-cycles, never wall-clock sleeps.
- SGB2 physical Game Link availability and routing through existing link topology boundaries.
- SNES/SFC-side data transfer and eventual 16-bit execution hooks.
- Save-state coverage for every live host state introduced by an implementation slice.

## Registers / MMIO

SGB does not add ordinary GB MMIO registers in the CGB sense. The primary GB-visible transport is the JOYP/P1 register path, where software writes packet bits through bits 4 and 5 using the SGB command protocol while still sharing the ordinary joypad register address.

The GB joypad subsystem should expose a narrow SGB host transport boundary rather than allowing the SGB host to poll arbitrary bus state. Ordinary P1 row selection and button reads remain joypad-owned; SGB packet collection and multiplayer host responses are SGB-host behavior layered at that boundary.

Physical serial link behavior remains serial/link-owned. Original SGB has no Game Link port, while SGB2 exposes physical Game Link support that must route through the existing link topology rather than through the SGB command-packet path.

## JOYP packet transport

SGB command transport uses active-low JOYP/P1 bits 4 and 5 as a host-side serial protocol. The host treats both lines high (`$30`) as idle, both lines low (`$00`) as the packet reset/start pulse, P14 low with P15 high (`$20`) as data bit `0`, and P14 high with P15 low (`$10`) as data bit `1`. Packet start is confirmed only by a `$00 -> $30` edge; one-low data states stage a pending bit and the bit is committed only when the lines return to `$30`, so repeated writes without the idle separator do not create extra bits and intermediate one-low transitions use the final one-low state before idle.

Each packet carries 16 bytes / 128 data bits, least-significant bit first within each byte, followed by a stop pulse that hardware-facing packet-edge ROMs treat as a delimiter even when its staged bit is `1`. The host records stop-bit `1` as an invalid-stop diagnostic and increments invalid packet counters, but it still completes the 16-byte command packet instead of rejecting otherwise valid commands solely for that stop value. The first byte encodes command ID in bits 3..7 and packet count in bits 0..2; the current host accepts packet counts `1..=7` and records packet-count `0` or impossible framing as invalid before any command-specific mutation. Multi-packet commands keep the active command ID and expected packet count in host state so later packets resume deterministically after save/load.

Packet transport state is part of `SgbHostSaveState`: last line state, explicit transport phase, pending staged data bit, active transfer flag, buffered bit/byte counts, current 16-byte buffer, pulse counters, invalid-stop counter, last packet trace, active command ID, expected/received packet counts, accepted command count, rejected packet count, invalid packet count, packet busy countdown, busy-rejected packet counters, `ICON_EN`-suppressed packet counters, and the last command IDs rejected or suppressed by those gates. Save/load must resume a partially buffered packet, pending start pulse, pending staged data bit, or a busy/suppressed host gate exactly instead of re-reading P1, sleeping, or replaying writes.

Before the first packet of a new command mutates host state, the decoder applies deterministic host gates. If `ICON_EN` bit 2 is set, the host records the packet as `SuppressedByIcon`, stores the suppressed command ID, and does not mutate palette, border, audio, OBJ, or backend state. If a `_TRN`/`OBJ_TRN` operation or later long host operation is busy, the host records the packet as `RejectedWhileBusy`, stores the busy-rejected command ID, and leaves the command accumulator idle. These windows are expressed in host frame/T-cycle-observable state and are not real-time sleeps.

`RealBoot` handoff is the one startup-owned exception to continuation of a partial packet: if the SGB boot ROM reaches `FF50` while a boot-private multi-packet command is still active, the host clears only the in-flight packet/command accumulator at that boundary so cartridge command transport starts from a clean idle state.

## Command ownership matrix

| Command family | Examples | Owning state | Implementation notes |
|---|---|---|---|
| Packet/control baseline | packet header, packet count, invalid packet handling | SGB host packet decoder | Decode before side effects; trace raw packets and command IDs. |
| Palettes | `PAL01`, `PAL23`, `PAL03`, `PAL12`, `PAL_SET`, `PAL_TRN`, `PAL_PRI` | SGB host palette state | Map DMG pixel shade indices through SGB palettes; do not use CGB palette RAM. |
| Attributes | `ATTR_BLK`, `ATTR_LIN`, `ATTR_DIV`, `ATTR_CHR`, `ATTR_TRN`, `ATTR_SET` | SGB host 20×18 attribute map and loaded attribute buffers | Keep separate from GB PPU tile metadata and CGB attributes. |
| Borders and masks | `MASK_EN`, `CHR_TRN`, `PCT_TRN` | SGB host border tiles, tilemap/attributes, palettes, mask state | Consume the shared 4 KiB transfer path and compose outside the GB LCD image. |
| OBJ transfer | `OBJ_TRN` | SGB host OBJ transfer state | Records enable/color-transfer control, palette IDs, copied OBJ palette data, OAM payload captures, and a short deterministic busy window; full SNES OBJ composition is outside the current architecture. |
| Multiplayer | `MLT_REQ` | SGB host controller mux | Support one/two/four-player modes and P1 player-ID cycling; separate from Game Link. |
| Host audio | `SOUND`, `SOU_TRN` | SGB host audio request/state | Keep ordinary GB APU separate; record deterministic diagnostics without audible SGB HLE or S-APU/S-DSP emulation. |
| System/menu control | `ATRC_EN`, `TEST_EN`, `ICON_EN` | SGB host system-control state and packet gate | Persist and expose attraction/test/menu disable state; `ICON_EN` bit 2 suppresses later command packets explicitly rather than acting as a silent no-op. |
| System/data/execution | `DATA_SND`, `DATA_TRN`, `JUMP` | SNES/SFC host backend request/state | Record diagnostics only; do not claim SNES RAM, VRAM-transfer targets, or 65C816 execution without a complete SNES/SFC emulator. |

The Slice 0 hardening contract defines typed `SgbHostBackendRequest` diagnostics for non-goal Slices 7-9: `SOUND` records an inline host-audio request, `SOU_TRN` uses the shared 4 KiB VRAM-transfer seam and records a sound-transfer descriptor, `DATA_SND` records an inline SNES memory write descriptor, `DATA_TRN` uses the same 4 KiB transfer seam with an explicit SNES destination, and `JUMP` records PC/NMI handler addresses and marks host execution as requested. The deterministic backend records these requests for save/load and tests; it does not emulate S-APU mixing, SNES RAM contents, or 65C816 instruction execution.

The post-Slice-6 command hardening closes the remaining documented one-packet command IDs while preserving the Slice 7-9 non-goal boundary. `ATRC_EN` and `TEST_EN` are persisted observable host state, `ICON_EN` persists menu-disable flags and bit-2 packet suppression, and `OBJ_TRN` records an explicit host OBJ transfer request/state through the same display-transfer seam even though full SNES OBJ composition is outside the current architecture.

## Video and color composition

The GB PPU still produces the 160×144 DMG LCD image with DMG pixel/shade information and sprite/background composition according to the GB core. The SGB host then maps those DMG shade indices through SGB palette and attribute state and places the resulting image inside the host SNES/SFC output with optional border graphics.

Slice 2 defines the first visible host-color path as a 160×144 LCD RGB555 composition result. This output is not CGB palette RAM and does not change the GB PPU framebuffer; it maps the already-produced DMG panel shade index through the SGB host's current screen palette 0 until later attribute commands select different palettes per 20×18 cell.

The SGB host stores four screen palettes, each with four RGB555 colors in the same bit layout used by SGB/SNES command payloads: bit 15 ignored, bits 0..4 red, bits 5..9 green, bits 10..14 blue, little-endian in command packets. Initial active SGB/SGB2 host state seeds physical screen palette 0 from the SGB BIOS built-in default palette, while the other physical screen palettes remain deterministic DMG grayscale until commands or transferred palette data replace them; handheld hosts keep deterministic DMG grayscale and no SGB color output.

SGB/SNES color index 0 is a shared transparent/backdrop color, not an independently visible color in each palette. The HLE host tracks the most recent application/SNES color-0 assignment as explicit `backdrop_color` for transparent border pixels, while LCD shade 0 resolves through the active screen-palette color-0 state unless a player-selected palette override is currently visible. This keeps cartridge border transfers from accidentally recoloring the already-composed GB LCD image: border color index 0 outside the GB LCD window resolves through the application backdrop, and border color index 0 inside the GB LCD window remains the transparent seam that lets the composed GB LCD image show through.

The post-Slice-2 title-palette refinement models the SGB BIOS default/title palette seed as host startup behavior. When cartridge header application rejects SGB command acceptance because the header is not `$0146 == $03` with old licensee `$014B == $33`, the HLE host compares the raw 16-byte header title against the known exact NUL-padded SGB BIOS title table and seeds physical screen palette 0 from the matching built-in palette. If no exact title match is found, or if the cartridge is SGB-command-capable, palette 0 remains the SGB BIOS default palette until implemented palette commands replace it. This is not CGB palette RAM, not GB PPU state, and not a per-game frontend hack.

Slice 2 implements the direct one-packet palette commands `PAL01`, `PAL23`, `PAL03`, and `PAL12`. Each command updates one screen color-0 assignment shared by all four visible screen palettes, three colors for the first targeted palette, and three colors for the second targeted palette. `PAL_SET`, `PAL_TRN`, and `PAL_PRI` remain later-slice commands because they depend on system palette transfer/selection and priority behavior outside the base direct-palette path. Slice 4's `PAL_SET` copies colors 1-3 from each selected logical system palette into the corresponding visible screen palette, but the shared visible LCD color 0 is taken from the selected physical SGB palette 0 entry; later palette IDs must not overwrite that shared screen color 0. Donkey Kong (GB) relies on this distinction when it selects a beige palette 0 plus later palettes whose logical color 0 is magenta-like.

Slice 3 adds the shared 4 KiB `_TRN` capture seam, and post-Slice-6 hardening gives it a deterministic multi-frame transfer timeline. `CHR_TRN`, `PCT_TRN`, `PAL_TRN`, `ATTR_TRN`, `SOU_TRN`, and `DATA_TRN` are still normal one-packet SGB commands, but completing the packet only schedules a host-side transfer request and opens a five-frame busy window. Across the documented capture window the machine reconstructs the 4 KiB payload from the prepared GB transfer display when the transfer layout remains valid (BG enabled, no scroll, identity `BGP`, and the selected tilemap/tiledata mode), including signed-tiledata transfer screens such as Donkey Kong (GB) and LCD-disabled tail frames such as Space Invaders where VRAM and layout registers remain prepared; otherwise it uses a deterministic raw fallback for HLE tests. The partial payload, phase, frame count, target, and final payload are all save-state-visible. The direct one-shot capture helper is crate-internal only and exists as a deterministic HLE/test seam, so public callers cannot bypass the production multi-frame path. This is not a CPU VRAM read, not CGB VRAM behavior, and not frontend-only postprocessing.

`CHR_TRN` decodes the command destination byte into tile block `$00-$7F` or `$80-$FF` and BG/OBJ tile type metadata, then writes the captured 4 KiB payload into SGB border tile memory. The BG/OBJ bit is retained as command metadata; the initial HLE border backend writes the same tile-memory range because Pan Docs notes that the bit appears not to change the address used by SGB software. Each SGB border tile is stored as 32 bytes of 4bpp SNES tile data: planes 0/1 interleaved by row, followed by planes 2/3 interleaved by row.

`PCT_TRN` decodes the captured payload as border screen data: bytes `$000-$6FF` hold the 32×28 visible SNES BG map, bytes `$700-$73F` hold the extra 29th row that can be visible as a flicker line on real output, and bytes `$800-$85F` physically carry three 16-entry little-endian RGB555 palette payloads corresponding to SNES BG palettes 4, 5, and 6. Color index 0 from these border-palette payloads is stored for observability but remains transparent to the current application backdrop; it does not become a palette-local visible border color, does not mutate the application backdrop, and does not mutate the active LCD screen-palette color 0. Pokémon Gold relies on this distinction during its startup border sequence, where a temporary PCT palette color 0 must not become a purple backdrop before the later `PAL_SET`. The payload must be the SGB transfer-display byte stream, not blindly the raw `$8000-$8FFF` backing range, because real software may arrange the visible transfer screen through signed tiledata at `$8800-$97FF`; the host stores 29 rows so the extra-row data survives save/load even though the current 256×224 composition surface displays only the 28 visible rows.

The full SGB host-frame output contract is `SgbHost::compose_frame_rgb555` / `Machine::sgb_framebuffer_rgb555`: a 256×224 RGB555 image with the 160×144 GB LCD window at `(48, 40)`. Border pixels outside the GB window come from the border tilemap, with color index 0 resolving through the shared backdrop; inside the GB window, border color index 0 lets the composed SGB LCD image show through, while non-zero border pixels cover the GB window. This models the SGB border as host state and preserves the GB PPU framebuffer as the source of the LCD image.

`MASK_EN` is host-video state. `Cancel` displays the live SGB LCD image, `Freeze` snapshots the current host LCD RGB555 image and continues using it, `BlankBlack` displays RGB555 black, and `BlankColor0` displays the current visible LCD screen-palette color 0. These modes affect the host LCD composition boundary and do not alter GB VRAM, GB PPU timing, DMG palette registers, or CGB palette hardware.

Slice 4 makes SGB screen attributes active host-side 20×18 tile-cell colorization state. `ATTR_BLK`, `ATTR_LIN`, `ATTR_DIV`, and `ATTR_CHR` mutate an explicit SGB attribute map whose cells select one of the four SGB screen palettes for the already-rendered DMG LCD shade; they must not be represented as CGB tile attributes, CGB palette indices, hidden GB VRAM state, or frontend-only postprocessing. This separation keeps CGB support and SGB support from contaminating each other.

`PAL_TRN` and `ATTR_TRN` share the multi-frame `_TRN` seam. `PAL_TRN` captures 4 KiB and decodes it as 512 system palettes of four little-endian RGB555 colors; this memory is not directly visible until `PAL_SET` copies selected logical palettes into the four physical SGB screen palettes. `ATTR_TRN` captures 4 KiB and stores the first 4050 bytes as 45 Attribute Files, each containing 90 packed bytes for a 20×18 two-bit palette-index map.

`PAL_SET` copies four little-endian system-palette IDs into physical palettes 0-3, records `PAL_PRI` priority state, and optionally applies an Attribute File or cancels a current `MASK_EN` state. `ATTR_SET` copies one Attribute File into the active 20×18 map and can also cancel `MASK_EN`. Invalid ATF IDs above `$2C` are retained as deterministic host diagnostics and do not mutate the active attribute map.

`PAL_PRI` is not sprite/background or per-pixel-source priority. It controls priority between the player's SGB menu-selected palette override and the application's palette/attribute commands. When a player palette override is active and `PAL_PRI` is disabled, application color commands continue updating host application state but the DMG window remains composed through the player-selected palette override. When `PAL_PRI` is enabled, the next application palette/attribute command (`PAL01`, `PAL23`, `PAL03`, `PAL12`, `ATTR_BLK`, `ATTR_LIN`, `ATTR_DIV`, `ATTR_CHR`, `PAL_SET`, or `ATTR_SET`) returns visible priority to the application's palette state. Transfer-only backing-data commands such as `PAL_TRN` and `ATTR_TRN` do not switch visible priority by themselves.

The SGB LCD composition path now selects a palette per 8×8 LCD cell before mapping the DMG shade to RGB555. `Freeze` snapshots the already colorized host LCD image, including the active attribute map; later palette or attribute changes do not alter the frozen image until mask cancellation. This keeps advanced screen coloring as a host-composition layer over the DMG framebuffer and leaves GB PPU fetch/render timing unchanged.

Borders belong to the SGB host. Static and dynamic borders update host border tile/palette/attribute state through SGB commands and the 4 KiB transfer path, then compose with the GB LCD image through the frontend-neutral 256×224 host-frame contract.

## Not planned host startup shell

The SNES/SFC-side startup shell owns the Super Game Boy logo animation, SGB jingle, startup transfer presentation, and built-in generic border shown before cartridge-side SGB border transfers. These effects are not contained in `sgb_boot.bin` / `sgb2_boot.bin`; those files are only GB-side low-window boot ROM images.

Slice 9 is not planned because experiments were inconclusive and a faithful startup shell depends on the broader SNES/SFC host environment: 65C816 execution, host firmware, S-APU/S-DSP behavior, SNES PPU/VRAM/CGRAM/OAM state, and deterministic coordination with cartridge-side `CHR_TRN`/`PCT_TRN`. If a future project reopens this with private `.sfc` firmware files, they remain local validation/extraction or execution inputs and are not committed assets.

Do not implement the generic border, logo animation, or jingle as frontend-only overlays or aesthetic HLE. Current `RealBoot` must remain limited to the GB-side boot ROM asset and implemented host command effects unless the project adopts a complete SNES/SFC host backend.

## Multiplayer

`MLT_REQ` enables SGB controller multiplexing for one, two, or four players. The SGB host owns player count, selected player, player-ID cycling, and routing of host input slots to GB-visible joypad reads. Frontends and test tooling should expose player slots without binding the GB core to any UI event model.

Frontend input routing for SGB/SGB2 must target the single SGB host controller slots, not the physical serial/link topology. In a single SGB-family desktop session, P2/P3/P4 host inputs are valid `MLT_REQ` controller sources even when original SGB correctly disables `EXT. PORT`; if SGB2 is placed in an explicit Game Link session, the linked-session P2 routing remains the physical secondary console and is separate from SGB host controller multiplexing.

The Slice 5 baseline treats P15 low-to-high transitions as the selected-player cycle edge when the active player count is even. This includes normal polling transitions and the transitions produced by SGB command packet transport itself, so sending `MLT_REQ` while multiplayer is already enabled can advance the selected player before the command side effect masks it into the new mode. P1 reads with both P14/P15 high return the player-ID nibble for multiplayer modes, while ordinary button and direction row reads are still resolved by the joypad subsystem using the selected SGB host input slot.

The `MLT_REQ` control byte values `$00`, `$01`, and `$03` map to one, two, and four players respectively. Control `$02` is invalid as a public mode but is preserved as a hardware-observed three-player/glitched selector state for SameSuite coverage: it does not cycle on later P15 rises and maps the current transport-cycled player index into the observed player 1/player 3 pair. Do not normalize this to a two-player or four-player mode in the core.

SGB multiplayer is not a DMG-07 four-player adapter and not a Game Link connection. SGB2 Game Link support is a physical serial/link feature and must remain separate from `MLT_REQ` controller multiplexing.

## Audio

The GB APU remains the shared core's ordinary audio generator. SGB special audio would be host-side behavior driven by `SOUND` and `SOU_TRN`, but general SGB special audio is not implemented in the current architecture.

The typed backend contract stores `SgbHostAudioRequest::Sound` for inline `SOUND` packets and `SgbHostAudioRequest::SoundTransfer` after the shared `_TRN` capture completes for `SOU_TRN`. These records are deterministic diagnostics and save-state-visible request seams, not an audible HLE fallback and not a real SGB audio backend. Donkey Kong (GB) and Animaniacs are historical diagnostic examples from inconclusive experiments; do not hardcode either title or advertise compatibility without a complete SNES/SFC host with firmware control flow, SPC700/S-APU execution, S-DSP state, BRR playback, and host-side scheduling.

## Timing / accuracy requirements

Use T-cycles as the GB core timing unit as elsewhere in the project, but keep SGB host profile timing explicit. `SGB NTSC`, `SGB PAL`, and `SGB2 NTSC` are represented as profile facts so video standard, corrected SGB2 clock behavior, and host presentation timing are not inferred from arbitrary names or frontend settings.

The Slice 6 baseline stores SGB profile clocks as rational profile facts rather than title-specific hacks: original SGB NTSC derives the GB master clock from a 21,477,272 Hz SNES/SFC source divided by 5, original SGB PAL derives it from a 21,281,370 Hz source divided by 5, and SGB2 NTSC uses a separate 20,971,520 Hz cartridge crystal divided by 5 for the corrected 4,194,304 Hz GB master clock. These facts describe host/wall-clock cadence and audio/video pitch; the core scheduler still advances deterministic logical T-cycles, and desktop frame pacing/speed reporting now consult the selected profile so original SGB NTSC targets about 61.17 GB frames/s, original SGB PAL targets about 60.61 GB frames/s, and SGB2/handheld profiles target about 59.73 GB frames/s. This is still the GB-side frame cadence; PAL host-video output at about 50 Hz remains a future SNES/SFC host-presentation axis rather than a reason to run the GB core at 50 FPS.

SGB2 corrected clock behavior belongs to the SGB2 profile. Original SGB may have PAL/NTSC host variants. SGB2 is NTSC in the planned public profile unless future evidence requires a broader matrix.

Packet decode, command side effects, transfer capture, controller multiplexing, and diagnostic backend request recording all require deterministic save/load continuation. `_TRN` and `OBJ_TRN` busy windows are frame/T-cycle-observable host state; commands sent during those windows must be rejected or suppressed through counters/traces, not delayed with wall-clock sleeps. Do not batch or approximate state in ways that prevent reproducing first divergence after restore.

## SGB2 Game Link

Original SGB has no physical Game Link port. SGB2 has Game Link support and routes through the existing `external_port`, `serial`, and `link::LinkedMachines` boundaries, not through the SGB command-packet layer.

SGB2 link availability is a profile/capability fact. `Machine::supports_external_port_attachment` rejects physical serial-port attachments for original SGB profiles, accepts them for SGB2, and `LinkedMachines::attach_dmg04_cable` validates that fact before installing the existing `DMG-04` cable topology. This keeps serial transfer semantics owned by the existing serial/link subsystems and keeps `MLT_REQ` controller multiplexing separate from physical Game Link behavior.

## SNES-side execution

`DATA_SND`, `DATA_TRN`, and `JUMP` require a host backend that can own SNES/SFC-side memory, data-transfer destinations, and execution state. The core represents these as typed `SgbSnesHostRequest` values: inline `DATA_SND`, 4 KiB transfer-backed `DATA_TRN`, and `JUMP` with PC/NMI handler addresses. In the current architecture these are diagnostic request records only; they do not emulate SNES RAM/VRAM ownership or 65C816 execution.

Space Invaders is a historical diagnostic example from inconclusive experiments, not a hardcoded special case and not a target for current slice closure. Reopen SNES-side execution only if the project adopts a complete SNES/SFC host environment with CPU, memory map, PPU/VRAM targets, interrupt behavior, firmware interactions, and synchronization with the embedded GB core.

## Dependencies

- [`info/MODEL-AXES.md`](../info/MODEL-AXES.md) for `HostPlatform`, operating-mode, and capability guidance.
- [`hardware/BOOT-ROM.md`](BOOT-ROM.md) for startup mode and boot-ROM asset policy.
- [`hardware/JOYPAD.md`](JOYPAD.md) for P1/JOYP ownership and button-read semantics.
- [`hardware/PPU.md`](PPU.md) for the DMG LCD image source consumed by SGB host composition.
- [`hardware/APU.md`](APU.md) for the GB APU boundary that SGB host audio must not replace.
- [`hardware/SERIAL.md`](SERIAL.md) and [`hardware/LINK.md`](LINK.md) for SGB2 Game Link routing.
- [`docs/roadmap/11-sgb.md`](../roadmap/11-sgb.md) for implementation sequencing.

## Primary references

- Pan Docs SGB Functions: https://gbdev.io/pandocs/SGB_Functions.html
- Pan Docs Specifications: https://gbdev.io/pandocs/Specifications.html
- Pan Docs SGB Command Packet: https://gbdev.io/pandocs/SGB_Command_Packet.html
- Pan Docs SGB Command Summary: https://gbdev.io/pandocs/SGB_Command_Summary.html
- Pan Docs SGB VRAM Transfers: https://gbdev.io/pandocs/SGB_VRAM_Transfer.html
- Pan Docs SGB Color Palettes Overview: https://gbdev.io/pandocs/SGB_Color_Palettes.html
- Pan Docs SGB Palettes: https://gbdev.io/pandocs/SGB_Command_Palettes.html
- Pan Docs SGB Borders: https://gbdev.io/pandocs/SGB_Command_Border.html
- Pan Docs SGB Multiplayer: https://gbdev.io/pandocs/SGB_Command_Multiplayer.html
- Pan Docs SGB Sound: https://gbdev.io/pandocs/SGB_Command_Sound.html
- Pan Docs SGB System Commands: https://gbdev.io/pandocs/SGB_Command_System.html
- Gekkio research and hardware references when SGB/SGB2 startup, clocks, boot ROMs, or ICD2 behavior need hardware confirmation.

## Tests

- Packet-decode unit tests for JOYP bit framing, packet counts, command IDs, malformed packets, reset behavior, packet busy/suppression gates, and partial-packet save/load.
- SameSuite SGB `command_mlt_req.gb` and `command_mlt_req_1_incrementing.gb` run as `console = "sgb"` multiplayer evidence now that Slice 5 implements `MLT_REQ`; the promoted report rows compare the raw 160×144 GB LCD framebuffer against fixtures materialized from the pinned gbdev/GBEmulatorShootout source manifest, so they validate the visible MLT_REQ result screens without claiming broader SGB host-frame or RealBoot closure.
- Mooneye `acceptance/boot_regs-sgb.gb` and `acceptance/boot_regs-sgb2.gb` run in the extra report as SGB/SGB2 `SkipBoot` public oracles for the direct-start register fingerprints `A=$01, C=$14` and `A=$FF, C=$14`; they do not claim SNES/SFC host startup-shell or SGB/SGB2 `RealBoot` closure.
- The cpp `sgb-ext-test.gb` packet-edge ROM runs as a promoted framebuffer fixture materialized from the pinned gbdev/GBEmulatorShootout source manifest and validates corrupt-stop, missing-idle, intermediate P14/P15 transition, mid-packet `$00`, and short-start behavior through the DMG LCD/tilemap result matrix.
- Boot/title palette tests proving SGB BIOS default/title seeding affects host RGB555 LCD output for command-rejected DMG software before any command packet, while SGB-command-capable titles and CGB framebuffer hardware remain unaffected.
- Palette composition tests proving direct `PALxx` commands affect host RGB555 LCD output without changing DMG PPU state or CGB palette state; attribute composition remains Slice 4.
- `PAL_PRI` tests proving player-selected palette overrides remain visible while priority is disabled, application palette/attribute commands regain visible priority when enabled, and transfer-only backing-data commands do not switch visible priority by themselves.
- Border transfer/composition tests for static border load, shared color-0/backdrop behavior, LCD and application-backdrop stability across `PCT_TRN`, repeated updates, mask behavior, and save/load continuation.
- `MLT_REQ` tests for one/two/four-player modes, player selection, P1 cycling, and frontend/test-runner input slots.
- SGB/SGB2 profile and startup tests for PAL/NTSC validity, corrected SGB2 clock profile, SGB/SGB2 direct-start register fingerprints with Mooneye SGB/SGB2 boot-register external rows, original SGB no-link behavior, and SGB2 Game Link availability.
- System-command tests for `ATRC_EN`, `TEST_EN`, `ICON_EN` packet suppression, `OBJ_TRN` state/OAM capture, and save/load coverage.
- Diagnostic backend request tests for deterministic `SOUND`, `SOU_TRN`, `DATA_SND`, `DATA_TRN`, and `JUMP` recording without claiming audible SGB audio or SNES-side execution.

Commercial titles are manual compatibility examples only unless a future private-suite policy explicitly changes that. Do not add them to CI or public gates.

## Implementation notes for this repo

- Keep SGB concerns out of unrelated DMG/CGB subsystems unless the subsystem owns the boundary being touched.
- The intended default shape is "shared GB core plus SGB host shell": the shared core owns CPU / PPU / APU / DMA / timer / bus truth, while the SGB layer owns packet interpretation, borders, colorization, multiplayer host behavior, and diagnostic host-backend request recording.
- SGB should reuse the DMG-family shared path through explicit axes and capabilities, not a dedicated "SGB core".
- SGB palette/attribute/border state must be explicit host state and must not piggyback on CGB palette or CGB tile-attribute internals.
- Every slice that adds live host state must update typed save states before the slice is considered closed.
- The Slice 0 baseline has an explicit inert `SgbHost` block in machine state, snapshots, and save states. It owns profile descriptors, deterministic diagnostic backend identity, video/multiplayer state, typed host-audio and SNES-side backend request contracts, and SGB2 capability facts. Slice 1 extends that block with startup mode, real-boot asset intent, header-derived command acceptance, JOYP packet decode state, command packet counters, and packet traces observed from `FF00` writes; the boot-ROM resolver now carries the same SGB asset identity through core, desktop loader helpers, and test-runner loading so RealBoot uses `sgb_boot.bin` or `sgb2_boot.bin` instead of `dmg_boot.bin`. Slice 2 adds persistent direct-palette state, a seven-packet command buffer for future multi-packet commands, and `SgbHost::compose_lcd_rgb555` / `Machine::sgb_lcd_framebuffer_rgb555` as the frontend-neutral base LCD color output; the post-Slice-2 title-palette refinement seeds palette 0 from the SGB BIOS default/title table when loading DMG-only headers that do not unlock SGB commands. Slice 3 adds pending `_TRN` transfer state, partial/final 4 KiB transfer payloads, border tile data, border tilemap/palette state, `MASK_EN` mask/freeze state, and `SgbHost::compose_frame_rgb555` / `Machine::sgb_framebuffer_rgb555` for the 256×224 host frame. Slice 4 adds active 20×18 screen attributes, packed ATF memory, 512 system palettes, indirect palette/attribute commands, `PAL_PRI`, and attribute-aware LCD/freeze composition; the post-Slice-4 `PAL_PRI` refinement adds player-selected palette override state and application-vs-player visible priority switching. Slice 5 adds `MLT_REQ` mode state, selected-player cycling, four SGB host input slots, player-ID P1 read overlay, and pending input-slot save-state coverage. Slice 6 moves SGB profile timing into the model/profile contract, persists selected profile metadata, and gates physical external-port attachments so original SGB has no Game Link while SGB2 routes through existing link topologies. Post-Slice-6 hardening adds `ATRC_EN`, `TEST_EN`, `ICON_EN`, `OBJ_TRN`, profile-aware direct-start registers, five-frame `_TRN` timing with partial payload save-state coverage, packet busy/rejection/suppression counters, edge-aware JOYP packet phases validated by `cpp/sgb-ext-test.gb`, and deterministic diagnostic request recording for `SOUND`, `SOU_TRN`, `DATA_SND`, `DATA_TRN`, and `JUMP`. Slices 7-9 are not implemented and not planned for the current architecture because strict special audio, uploaded SNES-side code, and real startup shell behavior require a complete SNES/SFC host emulator.

## Known pitfalls

- Naming the original Super Game Boy `SGB1` in user-facing docs or APIs when `SGB` / `Sgb` is sufficient.
- Treating SGB as CGB mode or reusing CGB palette/tile-attribute state for SGB colorization.
- Letting frontend-only presentation own border or palette behavior that should be deterministic core/host state.
- Hardcoding individual title colors in frontend code; SGB title palettes belong to the host BIOS table seed and only apply through exact header-title matching for command-rejected DMG software.
- Treating SGB border palette color index 0 as palette-local, letting a border `PCT_TRN` recolor LCD shade 0, or letting a border `PCT_TRN` replace the current application backdrop. Border color index 0 is transparent/backdrop state for border composition, outside-LCD border pixels with color index 0 must use the current application backdrop from palette commands, inside-LCD border pixels with color index 0 must remain transparent to the composed GB LCD image, and the LCD image keeps its active screen-palette color 0 until a screen palette command changes it.
- Treating `PAL_PRI` as sprite/background priority; its documented role is application-vs-player-selected palette priority.
- Mixing `MLT_REQ` controller multiplexing with DMG-07 or SGB2 Game Link semantics.
- Building title-specific HLE for `SOUND`, `SOU_TRN`, `DATA_SND`, `DATA_TRN`, `JUMP`, or startup-shell behavior and presenting it as SGB compatibility.
- Hardcoding commercial game behavior instead of implementing command/state semantics.

## Open questions

- RealBoot execution details inside the GB-side low-window mapping still need hardware validation: exact SGB/SGB2 post-boot handoff snapshots and boot timing.
- What frontend output surface should represent the combined SGB border plus GB LCD image without disrupting existing handheld framebuffer APIs.
