# Phase 11 — SGB/SGB2 implementation roadmap

Phase 11 brings Super Game Boy and Super Game Boy 2 support into the project as a host-shell expansion around the shared GB core. The goal is to prepare the architecture for SGB color, borders, multiplayer, special audio, SGB2 link behavior, and eventual SNES-side execution without forking CPU, PPU, APU, timer, DMA, bus, or cartridge logic.

## Scope and architecture rule

SGB is a DMG-compatible GB core hosted by an SGB/SNES shell. The GB core keeps owning SM83 execution, DMG-visible PPU/APU/timer/DMA/bus behavior, cartridge execution, and deterministic T-cycle scheduling; the SGB host owns JOYP packet interpretation, SGB command state, SNES/SFC border and screen composition, multiplayer controller multiplexing, host-audio events, SGB/SGB2 timing profile selection, and the eventual SNES-side code execution seam.

Do not model SGB as CGB mode, do not duplicate a second DMG core, and do not hide host-shell behavior behind generic DMG or CGB conditionals. Use `HostPlatform::Sgb` for the original Super Game Boy whenever possible, reserve `HostPlatform::Sgb2` for Super Game Boy 2, and route most behavior through explicit SGB capabilities instead of raw model checks.

The host implementation should be pluggable from Slice 0. Early slices may use a deterministic HLE SGB host for command effects, but the public interfaces must leave a real SNES-side backend possible for `DATA_SND`, `DATA_TRN`, `JUMP`, S-APU-related behavior, and titles such as Space Invaders.

## Public profiles and boot assets

| UI label | Machine profile | Host platform | GB core contract | Revision | Startup modes | RealBoot asset | Video standard | Notes |
|---|---|---|---|---|---|---|---|---|
| `SUPER GB` | `SGB` | `Sgb` | DMG-compatible | `SGB-CPU 01` | `skip-boot`, `real-boot` | `sgb_boot.bin` | `PAL` or `NTSC` | Original Super Game Boy host shell; no physical Game Link port. |
| `SUPER GB 2` | `SGB2` | `Sgb2` | DMG-compatible | `CPU SGB2` | `skip-boot`, `real-boot` | `sgb2_boot.bin` | `NTSC` | Corrected clock versus SGB and physical Game Link support. |

`MODEL: SGB` and `MODEL: SGB2` are frontend/manifest machine profiles resolved into the existing public axes, not permission to fork the GB core. `RealBoot` selects the SGB-profile-derived boot image and runs it through the normal CPU, bus, boot-overlay, and `FF50` machinery; `SkipBoot` synthesizes a coherent post-boot SGB/SGB2 handoff state and does not require boot ROM bytes.

Private boot ROM assets should use the same root-discovery style as existing boot-ROM work. A documented example root is `$HOME/emu/roms/bootrom`, containing `sgb_boot.bin` and/or `sgb2_boot.bin` when local RealBoot validation is enabled.

## Slice 0 — SGB architecture base

Scope: introduce the architectural seams and documentation contracts before any visible SGB command effects are implemented.

Implementation notes:

- Add explicit SGB host-shell state around the shared GB machine for packet reception, command dispatch, host video composition, controller multiplexing, host audio events, SGB/SGB2 profile data, and SNES-side memory/execution hooks.
- Keep the first host backend deterministic and HLE-friendly, but define the trait or boundary so a later 65C816/SNES backend can own SNES RAM, VRAM transfer targets, host audio, and `JUMP` execution without changing GB-core APIs again.
- Add SGB profile descriptors for `SGB NTSC`, `SGB PAL`, and `SGB2 NTSC`; encode SGB2 Game Link availability and corrected clock behavior in the profile rather than scattering special cases.
- Keep save-state metadata aware of host platform and SGB host state from the start, even if most host state is initially empty.

Acceptance criteria:

- `HostPlatform::Sgb` and `HostPlatform::Sgb2` are distinct from handheld mode and derive SGB host capabilities without changing DMG/CGB operating-mode semantics.
- A configured SGB/SGB2 machine can be constructed, reset, saved, and restored without visible SGB command effects and without regressing handheld DMG/CGB behavior.
- Documentation identifies ownership boundaries for GB core, SGB host, frontend presentation, boot assets, and future SNES execution.

Status: implemented as the Slice 0 baseline and later hardened to close the pluggable-host contract strictly. The core now has inert but explicit `SgbHost` state with `SgbNtsc`, `SgbPal`, and `Sgb2Ntsc` profile descriptors, a deterministic-HLE backend boundary, typed host backend requests for `SOUND`, `SOU_TRN`, `DATA_SND`, `DATA_TRN`, and `JUMP`, host-profile capability facts for SGB2 corrected clock and Game Link support, machine snapshot/save-state coverage, and construction/restore tests for `HostPlatform::Sgb` and `HostPlatform::Sgb2`. PAL profile selection started as a descriptor-only seam in Slice 0 and is exposed through the `MachineConfig` SGB profile path in Slice 6. The durable machine save-state format version is bumped because Slice 0 makes SGB host state part of the whole-machine payload.

## Slice 1 — Startup, unlock policy, and JOYP packet transport

Scope: make SGB/SGB2 startup and packet transport observable while keeping command effects mostly inert.

Implementation notes:

- Model SGB/SGB2 startup through `skip-boot` and `real-boot`; `real-boot` selects `sgb_boot.bin` for `SGB-CPU 01` and `sgb2_boot.bin` for `CPU SGB2`.
- Preserve cartridge-header SGB capability metadata and define the policy for SGB command acceptance when the header does or does not request SGB support.
- Decode SGB command packets transmitted through JOYP/P1 bits 4 and 5, including reset/pulse framing, byte/bit accumulation, packet count, command ID, and invalid/incomplete packet tracing.
- Add structured traces or snapshots for raw packet bytes before any command mutates palette, border, multiplayer, or audio state.

Acceptance criteria:

- SGB/SGB2 packet decode can be tested with synthetic JOYP writes and produces deterministic command records.
- Non-SGB handheld runs ignore the host command path and preserve ordinary joypad behavior.
- Save/load captures partial packet state exactly rather than reconstructing it from P1 reads.

Status: implemented as the Slice 1 baseline and later tightened to close the RealBoot asset resolver strictly. `SgbHost` now records startup mode, SGB/SGB2 real-boot asset intent, cartridge SGB header metadata, and command acceptance; active hosts accept command packets only when the loaded header advertises SGB support with `$0146 == $03` and old licensee `$014B == $33`. The host observes `FF00` writes at the machine boundary, decodes active-low P14/P15 packet pulses into 16-byte command records, traces complete/rejected/invalid/incomplete packets before side effects, and preserves partial packet state in whole-machine save states. `BootRomAssetKind` now derives handheld assets from `HardwareRevision` and SGB/SGB2 assets from `SgbHostProfile`, so core RealBoot reads, boot-ROM fingerprints, desktop loader helpers, and test-runner RealBoot loading use `sgb_boot.bin` for original SGB and `sgb2_boot.bin` for SGB2 instead of aliasing to `dmg_boot.bin`; SGB assets are `256`-byte low-window images with pinned SHA-256 validation. The durable machine save-state format version is bumped again because Slice 1 adds SGB-aware boot asset identity to boot save state and persisted boot-ROM asset payloads. The SameSuite SGB `command_mlt_req` and `command_mlt_req_1_incrementing` ROMs were added to `make test-roms` as informational `console = "sgb"` rows, ordered after `samesuite/ppu/blocking_bgpi_increase.gb`; at Slice 1 they were packet-visibility rows rather than multiplayer pass/fail requirements.

## Slice 2 — Base DMG color on SGB

Scope: implement the first visible SGB color path without using CGB palette hardware.

Implementation notes:

- Add host-side SGB palette RAM and map DMG pixel shade indices to SGB/SNES colors at the host composition boundary.
- Implement the basic palette commands needed for ordinary DMG-game colorization, keeping DMG `BGP`/`OBP` behavior owned by the GB PPU and SGB palette selection owned by the host shell.
- Expose a stable logical color output path suitable for screenshots, fixtures, and frontends before border composition is added.

Acceptance criteria:

- Base SGB palette changes affect final host output while the underlying GB LCD image and DMG palette registers remain correct.
- Save/load captures SGB palette RAM, active palette selection, and any palette priority state introduced by this slice.
- Example-title notes cover Alleyway, Super Mario Land 2: 6 Golden Coins, and Pokémon Red / Blue / Yellow as manual compatibility references, not gates.

Status: implemented as the Slice 2 baseline. `SgbHost` now owns four screen palettes of four SNES/CGB-layout RGB555 colors, an explicit base LCD palette selection, and a host-side 160×144 LCD RGB555 composition path that maps the DMG PPU's final panel shade indices without mutating DMG `BGP`/`OBP` behavior or enabling CGB palette hardware. The direct one-packet palette commands `PAL01`, `PAL23`, `PAL03`, and `PAL12` update the paired SGB screen palettes from little-endian RGB555 payload bytes; color 0 is shared by the two addressed palettes as documented by the command format, and bit 15 is masked as an ignored color bit. Command packet buffering now retains up to seven 16-byte packets in host command state so later multi-packet attribute/transfer commands can build on the same save-state boundary instead of replacing Slice 1 transport. The durable machine save-state format version is bumped again because Slice 2 adds live palette and command-buffer state. Manual compatibility examples for this slice are Alleyway, Super Mario Land 2: 6 Golden Coins, and Pokémon Red / Blue / Yellow. Automatic SGB BIOS title/default palettes for DMG-only games were intentionally treated as a post-Slice-2 refinement rather than part of the strict Slice 2 closure.

## Post-Slice 2 refinement — SGB boot/title palettes

Scope: model the SGB BIOS default and title palette seed for DMG software that does not unlock SGB commands, without contaminating CGB palette hardware or hardcoding frontend game overrides.

Implementation notes:

- Seed active SGB/SGB2 physical screen palette 0 from the SGB BIOS built-in default palette at host construction and cartridge-header application; leave the other screen palettes deterministic until commands or transferred palettes replace them.
- When the cartridge header is rejected for SGB command acceptance, match the raw 16-byte title bytes against the exact NUL-padded SGB BIOS title table and copy the selected built-in palette into screen palette 0.
- Do not apply title matching to SGB-command-capable cartridges; those start from the default boot palette and are expected to update visible color through `PALxx`, `PAL_SET`, `PAL_TRN`, or later real host execution.
- Preserve the same host palette save/load boundary as Slice 2: save states store the resulting palette RAM, not a frontend-only title override.

Acceptance criteria:

- Alleyway-style DMG-only title matching changes SGB RGB555 LCD output before any SGB command packet is received.
- Unknown or non-exact title matches fall back to the SGB BIOS default palette.
- SGB-command-capable headers with matching titles do not receive the automatic title palette.
- CGB framebuffer/palette hardware remains inactive for SGB title palette output.

Status: implemented as a post-Slice-2 refinement. `SgbHost` now owns the SGB BIOS built-in palette table and exact title-to-palette table as host startup data, applies palette 0 seeding during header application after command-acceptance policy is known, and preserves the resulting palette through the existing SGB palette save-state payload. No durable save-state format bump is required because the typed payload shape is unchanged.

## Slice 3 — VRAM transfer engine and borders

Scope: add the transfer path shared by SGB border and bulk-data commands, then render normal and dynamic borders.

Implementation notes:

- Implement the shared 4 KiB VRAM-transfer capture path used by `_TRN` commands as a host-side transfer, not as direct CGB VRAM behavior.
- Implement `MASK_EN`, `CHR_TRN`, `PCT_TRN`, and the static border pipeline with explicit SNES tile/attribute/palette storage in SGB host state.
- Support repeated border updates so games that animate or replace borders can do so without resetting the host.

Acceptance criteria:

- Border composition combines a 160×144 GB LCD image with the surrounding SGB border through a frontend-neutral output contract.
- Transfer and border state are included in save states and deterministic replay.
- Example-title notes cover Donkey Kong (GB), Animaniacs, Kirby’s Dream Land 2, and Killer Instinct as manual compatibility references.

Status: implemented as the Slice 3 baseline. `SgbHost` now records pending `_TRN` requests, captures the first 4 KiB of GB VRAM through a shared transfer buffer at the next PPU frame-start boundary, and decodes `CHR_TRN` and `PCT_TRN` into explicit host-owned border tile data, 29-row tilemap storage, and border palettes 4-6 without treating the payload as CGB VRAM. `MASK_EN` stores cancel/freeze/blank state in the SGB video block; freeze captures the current host LCD RGB555 image at command completion, while blank modes affect host LCD composition without mutating GB PPU state. A new 256×224 host-frame composition API combines border pixels and the 160×144 GB LCD window, using border color index 0 as the transparent window path and allowing non-zero border pixels to cover the window as on SGB. Repeated `CHR_TRN` and `PCT_TRN` calls overwrite the relevant host border state so static and dynamic border updates share the same path. The durable machine save-state format version is bumped again because Slice 3 adds live transfer, border, and mask/freeze state. Manual compatibility examples for this slice are Donkey Kong (GB), Animaniacs, Kirby’s Dream Land 2, and Killer Instinct.

## Slice 4 — Advanced screen coloring

Scope: implement SGB's per-region and per-character color attribute machinery.

Implementation notes:

- Implement `ATTR_BLK`, `ATTR_LIN`, `ATTR_DIV`, `ATTR_CHR`, `PAL_TRN`, `PAL_SET`, `ATTR_TRN`, `ATTR_SET`, and `PAL_PRI` as SGB host commands.
- Store the SGB 20×18 screen attribute map explicitly in host state; do not represent it as CGB BG attributes, CGB palette RAM, or hidden PPU tile metadata.
- Keep priority and mask behavior explicit so special screen coloring composes predictably with sprites, borders, and palette changes.

Acceptance criteria:

- Attribute commands update only SGB host colorization state and leave GB core tile fetch/render timing unchanged.
- Save/load captures attribute maps and command-loaded attribute buffers.
- Example-title notes cover Pokémon Yellow, Kirby’s Dream Land 2, and Balloon Kid as manual compatibility references.

Status: implemented as the Slice 4 baseline. `SgbHost` now owns an explicit 20×18 SGB attribute map, packed 45-entry ATF memory, 512 logical system palettes, and `PAL_PRI` state without using CGB palette RAM, CGB tile attributes, or hidden GB PPU metadata. `ATTR_BLK`, `ATTR_LIN`, `ATTR_DIV`, and `ATTR_CHR` update only the host attribute map; multi-packet attribute payloads reuse the Slice 2 packet buffer by flattening first-packet data bytes plus full subsequent packet data. `PAL_TRN` and `ATTR_TRN` reuse the Slice 3 4 KiB transfer seam, while `PAL_SET` and `ATTR_SET` copy logical palette/attribute files into visible host state and can cancel `MASK_EN` as documented. LCD and full-frame SGB composition now choose the screen palette per 8×8 GB-window cell before mapping the DMG shade to RGB555, and freeze snapshots the attribute-colored LCD image. The durable machine save-state format version is bumped again because Slice 4 adds live attribute maps, ATF buffers, system palettes, and palette-priority state. Manual compatibility examples for this slice are Pokémon Yellow, Kirby’s Dream Land 2, and Balloon Kid.

## Slice 5 — MLT_REQ multiplayer

Scope: support SGB controller multiplexing for two-player and four-player SGB games.

Implementation notes:

- Implement `MLT_REQ` modes for one, two, and four players and model P1 joypad-ID cycling as SGB host behavior.
- Add frontend and test-runner input-slot contracts for players 1 through 4 without coupling GB core joypad state to a specific UI backend.
- Keep SGB multiplayer separate from the DMG-07 adapter and from SGB2 Game Link; these are different physical routes.

Acceptance criteria:

- Synthetic tests prove player selection, player cycling, and per-player button reads are deterministic under `MLT_REQ`.
- Save/load captures current multiplayer mode, selected player, and input-slot state needed for continuation.
- Example-title notes cover Wario Blast / Bomberman GB as manual compatibility references.

Status: implemented as the Slice 5 baseline. `SgbHost` now handles the `MLT_REQ` command as host-owned controller multiplexing, with one-player, two-player, four-player, and hardware-observed control-2 behavior represented explicitly instead of routing through DMG-07 or SGB2 link state. P15 low-to-high transitions cycle the selected SGB player when the active player count is even, including the transitions generated by SGB packet transport itself, and P1 reads with both rows deselected expose the selected-player ID while ordinary button/direction row reads route through the selected host input slot. `Machine::set_sgb_joypad_button_pressed` exposes frontend/test-runner input slots for players 1 through 4, while `Machine::set_joypad_button_pressed` continues to feed player 1 on SGB machines and the ordinary handheld joypad elsewhere. Save states now include multiplayer mode, selected player, per-player input masks, and pending SGB input-slot changes; the durable machine save-state format version is bumped again because Slice 5 adds live controller-multiplexing state. Synthetic tests cover valid modes, P1 cycling, the SameSuite-relevant control-2 quirk, per-player input routing, and save/load continuation. Manual compatibility examples for this slice are Wario Blast / Bomberman GB.

## Slice 6 — SGB/SGB2 profiles and SGB2 Game Link

Scope: close model/profile differences that should not be bolted on after command support.

Implementation notes:

- Make `SGB NTSC`, `SGB PAL`, and `SGB2 NTSC` timing/profile data explicit in the SGB host profile and document which clock domains belong to the GB core versus the SNES/SFC host.
- Model SGB2's corrected clock versus SGB as a profile-level timing fact, not a title-specific hack.
- Route SGB2's physical Game Link support through the existing link topology and external-port boundaries; original SGB has no Game Link port.

Acceptance criteria:

- SGB2 link support composes with existing link/session infrastructure without reimplementing serial transfer semantics.
- SGB profile selection is persisted and rejects impossible combinations such as PAL SGB2 unless future hardware evidence requires otherwise.
- Tests distinguish original SGB no-link behavior from SGB2 Game Link availability.

Status: implemented as the Slice 6 baseline. `SgbHostProfile` now carries explicit timing facts for `SGB NTSC`, `SGB PAL`, and `SGB2 NTSC`: original SGB profiles derive their GB master clock from the SNES/SFC source divided by 5, while `SGB2 NTSC` uses the separate corrected 20,971,520 Hz cartridge crystal divided by 5 for the standard 4,194,304 Hz GB master clock. `MachineConfig` now carries an explicit SGB profile selection, `with_sgb_profile` exposes PAL/NTSC original SGB selection without inventing `SGB1`, and save-state metadata validates the selected profile so impossible or mismatched combinations such as PAL SGB2 do not restore into the wrong machine shape. Physical Game Link availability is now profile-gated: original SGB rejects external serial-port attachments, while SGB2 accepts them and `LinkedMachines::attach_dmg04_cable` reuses the existing `DMG-04` topology instead of implementing serial semantics inside the SGB host. The durable machine save-state format version is bumped again because Slice 6 adds persisted SGB profile metadata. Synthetic tests cover profile timing, profile/host-platform coherence, SGB no-link behavior, SGB2 link attachment, and direct external-port gating.

## Slice 7 — SGB special audio

Scope: implement SGB host-audio commands without compromising the GB APU model.

Implementation notes:

- Implement `SOUND` and `SOU_TRN` through the typed host-audio backend request seam owned by the SGB host.
- Keep ordinary DMG APU generation in the shared GB core; SGB special audio is mixed or exported by the host layer.
- The first backend may be deterministic HLE/event-driven, but the interface must leave room for a later S-APU/SNES audio implementation.

Acceptance criteria:

- Host-audio command state is observable, deterministic, and captured in save states.
- Audio output APIs can distinguish GB APU audio from SGB host audio without frontend-specific hacks.
- Donkey Kong (GB) special audio is documented as the main manual compatibility example.

## Slice 8 — SNES-side data transfer and 16-bit execution

Scope: implement the final pluggable host backend needed for SNES-side program execution.

Implementation notes:

- Implement `DATA_SND`, `DATA_TRN`, and `JUMP` against the typed SNES-side backend request boundary defined in Slice 0.
- Model SNES-side RAM/VRAM/data transfer ownership explicitly and avoid title-specific shortcuts for Space Invaders.
- Move beyond command-only HLE where required: this slice needs real or equivalently pluggable 65C816/SNES-side execution semantics for compatibility with code that runs on the host.

Acceptance criteria:

- Space Invaders is treated as a manual compatibility example for SNES-side execution, not as a hardcoded command path.
- Save/load captures SNES-side execution state needed for deterministic continuation.
- The implementation can still run earlier SGB color, border, multiplayer, and audio slices through the same host boundary.

## Cross-cutting save-state and determinism rule

Any slice that adds live SGB state must extend typed whole-machine save states before the slice is considered closed. Required state grows with the owning slice: Slice 1 adds packet accumulator state, Slice 2 adds SGB palette state, Slice 3 adds transfer and border state, Slice 4 adds attribute maps, Slice 5 adds multiplayer controller state, Slice 6 adds SGB profile/link state, Slice 7 adds host-audio state, and Slice 8 adds SNES-side execution state.

Restores must preserve hidden temporal state directly; do not reconstruct SGB host state from MMIO reads, frontend state, or replayed command logs.

## Reference-only commercial SGB examples

These titles are optional manual compatibility examples only. They are not hardware oracles, they do not define public pass/fail behavior, and they must not be added to CI or mandatory local gates unless a later private-suite policy explicitly supersedes this roadmap.

| Feature area | Example titles |
|---|---|
| Base SGB color for DMG games | Alleyway; Super Mario Land 2: 6 Golden Coins; Pokémon Red / Blue / Yellow |
| Normal/dynamic borders | Donkey Kong (GB); Animaniacs; Killer Instinct; Kirby’s Dream Land 2 |
| Advanced screen coloring | Pokémon Yellow; Kirby’s Dream Land 2; Balloon Kid |
| Special SGB audio | Donkey Kong (GB) |
| `MLT_REQ` multiplayer | Wario Blast / Bomberman GB |
| SNES-side execution | Space Invaders |

## API and interface rules

- Keep `MachineConfig` as the single entry point for model, operating mode, host platform, startup, and future SGB profile selection.
- Prefer `CapabilitySet` for behavior gates such as SGB enhancements, SGB multiplayer, SGB border composition, SGB2 link availability, and SNES-host execution availability.
- Keep SGB palette/attribute/border state out of the CGB palette and tile-attribute paths.
- Keep SGB multiplayer out of DMG-07 and SGB2 Game Link routing; SGB controller multiplexing and physical serial links are different subsystems.
- Do not create a second GB core or duplicate CPU/PPU/APU/timer/DMA implementations for SGB.

## Test plan

- Every SGB implementation slice must preserve the existing DMG ROM gate and run relevant CGB gates when shared model, boot, PPU, APU, serial, scheduler, link, save-state, or frontend output contracts are touched.
- Add focused synthetic/unit tests per slice for packet decode, command state, boot/title palette seeding, palette/attribute composition, border composition, `MLT_REQ` controller cycling, SGB2 link routing, save/load continuation, audio events, and SNES host seams.
- Add manifest or frontend smoke support only after an oracle channel and artifact policy are defined; commercial games listed here remain manual examples.
- Use SameBoy and other mature SGB-capable emulators only as comparison aids after primary documentation and hardware research, not as hardware truth by themselves.

## Assumptions

- The original Super Game Boy should be named `SGB` / `Sgb` rather than `SGB1` / `Sgb1` wherever possible.
- `SGB2` / `Sgb2` remains explicit because Super Game Boy 2 differs in clock behavior and Game Link availability.
- Early host behavior may be HLE, but the interfaces must not block later real SNES-side execution.
- Commercial ROM examples remain manual compatibility references rather than gates.
