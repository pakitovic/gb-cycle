# Model Axes

## Scope

Explain how to use the public model-facing types introduced around `ConsoleModel`, `OperatingMode`, `HostPlatform`, and `CapabilitySet`.

This file is a code-facing usage and migration note. It exists to keep DMG, CGB, and SGB work from collapsing distinct concepts back into one catch-all enum.

## Authority boundaries

- [`ARCHITECTURE.md`](../ARCHITECTURE.md) owns the existence of the separate axes and the high-level architectural reason for them.
- [`hardware/CGB.md`](../hardware/CGB.md), [`hardware/SGB.md`](../hardware/SGB.md), and [`hardware/BOOT-ROM.md`](../hardware/BOOT-ROM.md) own the subsystem behavior that consumes those axes.
- This file owns the global model-profile reference table that aligns the public axes with hardware-profile names, without making those hardware-profile names functional behavior gates.
- This file owns the practical "which type should I consult here?" guidance for production code and follow-up refactors.

If this file conflicts with a subsystem handbook about hardware truth, the subsystem handbook wins.

## Mental model

Treat the public model surface as independent axes, with firmware image selection derived from the hardware revision instead of configured as a separate public axis:

```text
ConsoleModel     = visible product model selected by users and frontends
HardwareRevision = CPU/revision profile selected within that model; RealBoot derives firmware from it
OperatingMode    = which GB-visible mode the software is currently running under
HostPlatform     = which outer host shell surrounds the shared GB core
SgbHostProfile   = which SGB/SGB2 host shell timing/link profile applies when HostPlatform is Sgb/Sgb2
```

Examples:

- `ConsoleModel::GameBoy` + `HardwareRevision::DmgCpuC` + `OperatingMode::Dmg` + `HostPlatform::Handheld` = active ordinary Game Boy profile with the standard DMG boot ROM derived for `RealBoot`
- `ConsoleModel::GameBoy` + `HardwareRevision::DmgCpu0` = the same visible Game Boy product model with the active earlier DMG0 firmware profile for targeted boot-register and DIV validation
- `ConsoleModel::GameBoyPocket` or `ConsoleModel::GameBoyLight` + `HardwareRevision::CpuMgb` + `OperatingMode::Dmg` = DMG-family handheld product using the MGB boot/direct-start profile
- `ConsoleModel::GameBoyColor` + `HardwareRevision::CpuCgbE` + `OperatingMode::Cgb` = CGB-family silicon on the active CGB-E profile, with `cgbE_boot.bin` selected automatically for `RealBoot`
- `ConsoleModel::GameBoyAdvance` + `HardwareRevision::CpuAgb0` or `HardwareRevision::CpuAgbA` + `OperatingMode::Cgb` = AGB CGB-compatible silicon on an active GBA-enhanced profile, with `cgb_agb0_boot.bin` or `cgb_agb_boot.bin` selected automatically for GB/C `RealBoot` and no native `gba_bios.bin` requirement in this core
- `ConsoleModel::GameBoyColor` + `OperatingMode::GbCompatible` = CGB-family silicon running monochrome software-visible mode
- `ConsoleModel::GameBoyColor` + `OperatingMode::CgbDmgExt` = experimental CGB-family silicon running a DMG software contract with a narrow DocBoy `dmg_ext_mode`-style register profile, not full PGB/PSM support
- `HostPlatform::Sgb` or `HostPlatform::Sgb2` = SGB shell around the shared GB core, not a different GB silicon family
- `SgbHostProfile::SgbNtsc`, `SgbHostProfile::SgbPal`, or `SgbHostProfile::Sgb2Ntsc` = the concrete SGB/SGB2 host profile used for video standard, source clock, corrected-clock fact, and physical Game Link availability; `SgbPal` is only coherent with `HostPlatform::Sgb`, and `Sgb2Ntsc` is only coherent with `HostPlatform::Sgb2`
- `DmgCpu0` is active only for handheld `GameBoy` profiles; SGB/SGB2 profiles share the `GameBoy` console family but expose only their profile-backed `DmgCpuC` GB core so handheld DMG0 boot handoff corrections do not contaminate SGB startup state.

`CapabilitySet` is the derived semantic view over the broad model axes. SGB host-profile facts currently live on `SgbHostProfile` because they are profile-specific timing/link facts rather than GB-silicon behavior; code that needs SGB2 Game Link availability or corrected clock should consult the selected SGB profile instead of duplicating `HostPlatform` checks.

## Reference model profiles

This table is an informative reference for aligning the public axes with the hardware profile names used in research notes and user-facing documentation. `HardwareRevision` now models the DMG/MGB/CGB/AGB CPU revision profiles listed below, but only a subset is active in frontends and manifests: `DmgCpu0` and `DmgCpuC` for `GameBoy`, `CpuMgb` for `GameBoyPocket` / `GameBoyLight`, `CpuCgb0` / `CpuCgbC` / `CpuCgbD` / `CpuCgbE` for `GameBoyColor`, and `CpuAgb0` / `CpuAgbA` for `GameBoyAdvance`. Rows that do not have current Rust enum variants remain forward-looking documentation-only. Revision defaults, active revision sets, derived firmware filenames, and `SkipBoot` profiles remain owned by [`hardware/BOOT-ROM.md`](../hardware/BOOT-ROM.md#product-and-firmware-profiles).

| Default | Console Model | Host Platform | CPU | Boot ROM | Operation Mode | Color Mode | Info |
|---:|---|---:|---|---|---|---|---|
| false | Game Boy | Handheld | `DMG-CPU 0` | `dmg0_boot.bin` | DMG | DMG green palette | Initial DMG-CPU 0 unit; active for explicit `dmg-cpu-0` boot validation but not the default Game Boy revision. |
| false | Game Boy | Handheld | `DMG-CPU A` | `dmg_boot.bin` | DMG | DMG green palette | Later DMG revision; standard DMG boot ROM. |
| false | Game Boy | Handheld | `DMG-CPU B` | `dmg_boot.bin` | DMG | DMG green palette | Common DMG revision; standard DMG boot ROM. |
| true | Game Boy | Handheld | `DMG-CPU C` | `dmg_boot.bin` | DMG | DMG green palette | Late DMG revision; standard DMG boot ROM. |
| true | Game Boy Pocket | Handheld | `CPU MGB` | `mgb_boot.bin` | DMG | MGB gray palette | DMG-class mode with MGB boot; final A register value `$FF` enables software detection. |
| true | Game Boy Light | Handheld | `CPU MGB` | `mgb_boot.bin` | DMG | MGL light palette | DMG-class mode with MGB boot; MGL distinction is the light/backlit display profile. |
| false | Game Boy Color | Handheld | `CGB-CPU 0` | `cgb0_boot.bin` | CGB; GB Compatible on CGB; CGB DMG-ext experimental | CGB color; GB with CGB palettes | Initial CPU without suffix; active for explicit `cpu-cgb-0` boot validation; boot ROM does not initialize wave RAM. |
| false | Game Boy Color | Handheld | `CPU CGB A` | `cgb_boot.bin` | CGB; GB Compatible on CGB; CGB DMG-ext experimental | CGB color; GB with CGB palettes | Early CGB revision; pre-D family, keep CGB timing/APU quirks distinct from D/E. |
| false | Game Boy Color | Handheld | `CPU CGB B` | `cgb_boot.bin` | CGB; GB Compatible on CGB; CGB DMG-ext experimental | CGB color; GB with CGB palettes | Common early CGB revision; pre-D family with known audio, double-speed, and LCD timing quirks. |
| false | Game Boy Color | Handheld | `CPU CGB C` | `cgb_boot.bin` | CGB; GB Compatible on CGB; CGB DMG-ext experimental | CGB color; GB with CGB palettes | Last pre-D CGB-family revision; known APU/audio-register, double-speed, and LCD timing quirks. |
| false | Game Boy Color | Handheld | `CPU CGB D` | `cgb_boot.bin` | CGB; GB Compatible on CGB; CGB DMG-ext experimental | CGB color; GB with CGB palettes | Post-C family revision; fixes many A/B/C-era issues and changes LCD/PPU timing behavior. |
| true | Game Boy Color | Handheld | `CPU CGB E` | `cgbE_boot.bin` | CGB; GB Compatible on CGB; CGB DMG-ext experimental | CGB color; GB with CGB-E boot profile | Default CGB revision; CGB-CPU-06 integrates WRAM into the CPU, uses the distinct `cgbE_boot.bin`, and owns the CGB-E extra-OAM readback used by `which.gb`. |
| true | Super Game Boy | Sgb | `SGB-CPU 01` | `sgb_boot.bin` | DMG-compatible hosted by SGB | SGB palettes + SNES/SFC border | SGB host; PAL/NTSC cases; DMG-class GB core with SGB boot/protocol handled through the SNES/SFC side; no physical Game Link port. |
| false | Super Game Boy 2 | Sgb2 | `CPU SGB2` | `sgb2_boot.bin` | DMG-compatible hosted by SGB2 | SGB palettes + SNES/SFC border | SGB2 host; NTSC case; corrected clock versus SGB; physical Game Link support; boot identifies SGB2 separately. |
| false | Game Boy Advance | Handheld | `CPU AGB` | `cgb_agb0_boot.bin` | GB/GBC Compatible on AGB0 | AGB color; GB/GBC with AGB0 profile | Active explicit `cpu-agb-0` revision for AGB0 GB/C boot validation; it is a `HardwareRevision` under the `GameBoyAdvance` model, not a separate `ConsoleModel`, and currently shares the `CpuAgbA` direct-start behavior. Native `gba_bios.bin` is outside the GB/C core startup path. |
| true | Game Boy Advance | Handheld | `CPU AGB A` | `cgb_agb_boot.bin` | GB/GBC Compatible on AGB | AGB color; GB/GBC with AGB profile | Default `GB ADVANCE` revision; CGB-compatible boot exposes GBA-enhanced detection to software through the AGB post-boot register fingerprint. Native `gba_bios.bin` is not loaded by this GB/C core path. |
| false | Game Boy Advance SP | Handheld | `CPU AGB B` | `cgb_agb_boot.bin` | GB/GBC Compatible on AGB | AGB/AGS color; GB/GBC with AGB profile | Documentation-only hardware row; gb-cycle intentionally exposes one `GB ADVANCE` UI model for GBA-enhanced GB/C behavior rather than separate SP UI variants. |
| false | Game Boy Advance SP | Handheld | `CPU AGB B E` | `cgb_agb_boot.bin` | GB/GBC Compatible on AGB | AGB/AGS color; GB/GBC with AGB profile | Documentation-only late AGS/AGS-101 row; keep full GBHWDB CPU label without adding a separate frontend model. |
| false | Game Boy Micro | Handheld | `CPU AGB E` | `gba_bios.bin` | AGB | AGB/OXY color | OXY-family CPU; GBA-only cartridge compatibility, with no physical GB/GBC compatibility and no GB/C core profile. |
| false | Game Boy Player | Gbs | `CPU AGB A` | `cgb_agb_boot.bin` | GB/GBC Compatible on AGB | AGB/CGB color output via GameCube video path | Documentation-only host row; gb-cycle does not expose a `GB PLAYER` UI model because the current GB/C core uses the same GBA-enhanced AGB-compatible register profile as `GB ADVANCE`. |
| false | Game Boy Player | Gbs | `CPU AGB A E` | `cgb_agb_boot.bin` | GB/GBC Compatible on AGB | AGB/CGB color output via GameCube video path | Documentation-only late DOL-GBS row; keep full GBHWDB CPU label without adding a separate frontend model. |

## When to use each type

### Use `ConsoleModel` when the question is about the visible product model

Reach for `ConsoleModel` when the code needs to know the user-facing product class and its default hardware-family contract.

Typical uses:

- default boot ROM kind and allowed firmware set
- skip-boot power-up presets that differ by product model
- silicon-family quirks such as DMG-family-only OAM corruption
- product-specific desktop presentation such as DMG, MGB, Game Boy Light display palette, or the `GB ADVANCE` model label
- physical product facts such as CGB infrared being present on `GameBoyColor` but absent from `GameBoyAdvance` even though both are CGB-family for GB/C execution
- raw family classification when a subsystem is defining a derived capability

Do not branch on a revision simply because it exists in the enum. A `HardwareRevision` branch should correspond to a modeled firmware/direct-start contract or a tested hardware behavior; otherwise prefer the derived capability or model-level default.

Do not use `ConsoleModel` just because it is nearby in the API. If the real question is "is this feature enabled right now?", `ConsoleModel` is usually too low-level.

### Use `HardwareRevision` when the question is about CPU revision or revision-derived firmware

Reach for `HardwareRevision` when the code needs to know the selected CPU/revision profile, load or verify the `RealBoot` firmware image derived from that profile, persist save-state metadata, or gate a tested revision-specific silicon behavior. Do not add a separate user-facing boot-ROM-kind setting: `RealBoot` firmware filename, expected size, and expected SHA-256 are derived from `HardwareRevision`, while `SkipBoot` and `CustomBoot` do not read boot-ROM bytes.

The authoritative default/active revision matrix and the matching boot-ROM filenames live in [`hardware/BOOT-ROM.md`](../hardware/BOOT-ROM.md#product-and-firmware-profiles). Keep this file focused on which axis production code should consult, not on duplicating boot-profile data.

### Use `OperatingMode` when the question is about the active software-visible GB mode

Reach for `OperatingMode` when the machine's silicon is not enough to answer the question because the running mode matters.

Typical uses:

- CGB native mode versus CGB compatibility mode
- mode-dependent MMIO visibility or routing
- mode-dependent palette behavior
- mode-dependent boot handoff on CGB-family hardware
- policy that should differ between "CGB hardware running CGB software" and "CGB hardware running DMG software"
- experimental CGB DMG-ext policy where the software contract remains DMG-like but a small set of CGB-family registers stays visible for DocBoy `dmg_ext_mode` validation

Do not treat `OperatingMode::GbCompatible` as shorthand for DMG silicon. The software contract may look DMG-like while the underlying hardware family is still CGB.

Do not treat `OperatingMode::CgbDmgExt` as native CGB or as full PGB/PSM support. It is a CGB-family experimental mode with a DMG software contract, CGB silicon quirks, and only the documented narrow register profile enabled.

### Use `HostPlatform` when the question is about the outer shell, not the GB silicon

Reach for `HostPlatform` when the behavior belongs to the environment around the shared GB core.

Typical uses:

- SGB command transport
- SGB border ownership
- SGB multiplayer-host behavior
- host-shell timing coordination with a SNES-side implementation

`HostPlatform` should not decide CPU, PPU, DMA, timer, or APU truth directly unless a subsystem handbook later documents a real host-platform-visible effect.

Borrowed SGB border presentation for handheld models is not a `HostPlatform` change. The live DMG/MGB/LGB/CGB/AGB machine remains `HostPlatform::Handheld`; frontends may attach a presentation-only borrowed border extracted by a temporary SGB NTSC machine, and the 160×144 aperture must continue to come from the active handheld model.

Use `SgbHostProfile` when the host-shell question needs the specific SGB profile rather than merely "is this SGB?". Typical uses include choosing SGB NTSC versus SGB PAL presentation, identifying SGB2 corrected-clock behavior, selecting the SGB/SGB2 real-boot asset intent, and gating the physical Game Link port. `MachineConfig::with_sgb_profile` is the preferred profile-selection entry point because it keeps the host platform coherent with the profile.

### Use `CapabilitySet` by default for subsystem behavior gates

For most production code, prefer `CapabilitySet` over directly branching on `ConsoleModel`, `OperatingMode`, and `HostPlatform`.

Use it when the question is semantic:

- "does DMG software contract apply?"
- "are CGB extensions enabled?"
- "is the experimental CGB DMG-ext register subset enabled?"
- "do DMG-family silicon quirks apply?"
- "are SGB host enhancements active?"

This keeps subsystem code readable and avoids re-encoding the meaning of the axes differently in CPU, PPU, DMA, timer, and boot code.

## Preferred decision order

When adding or refactoring model-aware code:

1. Ask whether the code really wants a semantic capability.
2. If yes, use `CapabilitySet` or add a new derived capability there.
3. If not, ask whether the question is about silicon, active operating mode, or host shell.
4. Use `ConsoleModel`, `OperatingMode`, `HostPlatform`, or `SgbHostProfile` only for that specific raw concern.

In short:

```text
behavior gate -> CapabilitySet first
silicon fact  -> ConsoleModel
mode fact     -> OperatingMode
host-shell    -> HostPlatform
sgb profile   -> SgbHostProfile
```

## Concrete examples

Use `CapabilitySet::dmg_family_quirks_enabled()` for:

- DMG-family OAM corruption gating
- other true DMG-family-only silicon quirks

Use `ConsoleModel` directly for:

- deriving the default and active `HardwareRevision` set, including `CpuAgb0` and default `CpuAgbA` for the active `GameBoyAdvance` profile
- product-specific skip-boot startup presets
- product-specific desktop display palette selection

Use `HardwareRevision` directly for:

- boot ROM asset filenames, expected sizes, and hashes
- `RealBoot` firmware payload lookup after the model/revision pair is validated
- revision-specific hardware behavior with a concrete oracle, such as the CGB-E CH1 sweep-restart timing seam

Use `OperatingMode` or a capability derived from it for:

- whether CGB palette hardware is actively exposed
- whether CGB-only tile attributes participate in rendering
- whether a CGB running a DMG title should follow DMG-visible rendering rules
- whether the experimental CGB DMG-ext register subset should be exposed without enabling native CGB rendering

Use `HostPlatform` or a capability derived from it for:

- SGB packet decoder ownership
- SGB border composition outside the handheld LCD image
- SGB multiplayer-controller multiplexing

Use `SgbHostProfile` directly for:

- SGB NTSC versus SGB PAL source-clock and video-standard facts
- SGB2 corrected-clock and physical Game Link availability
- validating save-state restore against the selected SGB/SGB2 profile

## Migration guidance

Do not mass-rewrite the repo from `console_model` checks to the new axes in one pass.

Preferred migration strategy:

1. Leave stable DMG-only code alone unless the change already needs CGB/SGB awareness.
2. When touching a subsystem for new CGB or SGB work, identify whether each branch is really about silicon, mode, or host shell.
3. Replace only the branches that become ambiguous under CGB or SGB.
4. If multiple subsystems need the same semantic test, add or extend a `CapabilitySet` query instead of duplicating that logic.

This keeps behavior-neutral refactors small and makes later CGB bring-up easier to review.

## Anti-patterns

- Do not use `ConsoleModel::GameBoy` as a synonym for "DMG-visible behavior".
- Do not use `OperatingMode::GbCompatible` as a synonym for "DMG-family silicon".
- Do not use `OperatingMode::CgbDmgExt` as a synonym for native CGB, full PGB, PSM NMI, or live post-boot `OPRI` visual switching.
- Do not put SGB host-shell policy behind random `ConsoleModel` checks.
- Do not treat `HostPlatform::Sgb2` as enough to answer NTSC/PAL or corrected-clock questions once a concrete SGB profile is available.
- Do not re-derive the same semantic meaning from the raw axes in several subsystems.
- Do not add a second emulator path just because one raw axis is insufficient.

## Review checklist

When a change adds model-aware behavior, verify:

- the branch is using the right axis
- semantic gates prefer `CapabilitySet`
- silicon-only quirks are not accidentally keyed off `OperatingMode`
- host-shell behavior is not leaking into handheld-core logic
- CGB compatibility mode is not being confused with DMG silicon
- experimental CGB DMG-ext behavior is gated explicitly and does not silently broaden native CGB, PGB, or PSM behavior
