# CGB

## Scope

Own the Color Game Boy silicon-extension contract inside `gb-core`: CGB model axes, boot-selected operating mode, CGB-only MMIO visibility, double-speed domains, VRAM/WRAM banking, RGB555 palette and framebuffer surfaces, GDMA/HDMA integration boundaries, CGB serial speed, infrared register ownership, CGB-family APU taps, CGB compatibility mode, and the narrow experimental CGB DMG-ext profile.

Do not duplicate per-subsystem mechanics here. Boot assets and handoff state live in [`BOOT-ROM.md`](BOOT-ROM.md); model-axis usage lives in [`../info/MODEL-AXES.md`](../info/MODEL-AXES.md); scheduler vocabulary lives in [`../info/TIMING-AND-ACCURACY.md`](../info/TIMING-AND-ACCURACY.md); memory, PPU, DMA, APU, serial, link/IR, cartridges, and bus rules live in [`MEMORY.md`](MEMORY.md), [`PPU.md`](PPU.md), [`DMA.md`](DMA.md), [`APU.md`](APU.md), [`SERIAL.md`](SERIAL.md), [`LINK.md`](LINK.md), [`CARTRIDGES-MBC.md`](CARTRIDGES-MBC.md), and [`BUS.md`](BUS.md); ROM-suite operation lives in [`../info/ROM-SUITES.md`](../info/ROM-SUITES.md); validation policy lives in [`../TESTING.md`](../TESTING.md); source ordering lives in [`../REFERENCES.md`](../REFERENCES.md); open work belongs in [`../TODO.md`](../TODO.md) or [`../ROADMAP.md`](../ROADMAP.md).

## Design rule

CGB is an extension of the shared Game Boy core, not a second emulator. Shared subsystems must keep DMG-family behavior, CGB-family silicon behavior, and active software-visible operating mode separate enough that CGB native mode, CGB compatibility mode, experimental CGB DMG-ext mode, and future host shells can reuse the same scheduler and bus architecture.

Prefer explicit hardware-facing state over convenience flags: `ConsoleModel` identifies the visible product family, `HardwareRevision` identifies revision/firmware profile, `OperatingMode` identifies the running software contract, and `CapabilitySet` is the preferred semantic gate when production code needs a derived capability. Do not branch on a CGB revision merely because the enum has a variant; add revision gates only for validated behavior.

The shared scheduler T-cycle remains the CPU-visible unit. CGB double speed changes which hardware domains tick on a scheduler cycle; it must not become a global multiplier for frames, lines, dots, audio, serial, RTC, or host output.

## Model axes and operating modes

- `ConsoleModel::GameBoyColor` is the CGB-family silicon entry point. The active revision set is `CpuCgbC`, `CpuCgbD`, and `CpuCgbE`, with `CpuCgbE` as the default and revision-derived firmware names owned by [`BOOT-ROM.md`](BOOT-ROM.md).
- `OperatingMode::Cgb` enables native CGB extensions: banked VRAM/WRAM, native palette RAM, CGB tile attributes, GDMA/HDMA, speed switching, CGB serial speed, infrared register state, and CGB-only readbacks where owned by the relevant subsystem.
- `OperatingMode::GbCompatible` means CGB-family silicon running a DMG software-visible contract. It is not DMG silicon: DMG-family-only quirks remain disabled, while documented boot-visible CGB readbacks and CGB compatibility RGB555 presentation remain CGB-family state.
- `OperatingMode::CgbDmgExt` is an experimental DMG-software contract with a narrow CGB-register profile for targeted validation. It is not native CGB, not PGB/PSM support, and not permission to enable native tile attributes, native palette data mutation, HDMA, boot-ROM remap side effects, external LCD behavior, or live post-boot visual priority changes without evidence.
- CGB-E-specific behavior is allowed only where there is a concrete oracle or hardware-facing observation, such as the current extra-OAM readback and long CH1 sweep-restart hold. Do not use `CpuCgbE` as a generic accuracy upgrade switch.
- Desktop presentation choices are frontend-owned. CGB rendering consumes the core RGB555 framebuffer directly; DMG/MGB/LGB palette selectors in the desktop frontend must not rewrite CGB core color state.

## MMIO ownership matrix

CGB-only MMIO must be routed through typed owners, not through generic `FFxx` storage. DMG-family models read unavailable CGB-only registers as `$FF` and ignore writes unless an owning document states a shared behavior.

| Surface | Owner | Native `Cgb` | `GbCompatible` on CGB | Experimental `CgbDmgExt` |
| --- | --- | --- | --- | --- |
| `KEY0` / `FF4C` | boot + CGB system state | Boot-owned before `FF50`; runtime readback `$FF`; writes ignored after lock | Same lock/readback contract | Same lock/readback contract, with experimental bit-3 interpretation only when the policy enables it |
| `KEY1` / `FF4D` | speed controller | Read/write prepare bit and current-speed bit | Unavailable `$FF`; speed stays normal | Enabled like native CGB for the narrow profile |
| `VBK` / `FF4F` | PPU/bus VRAM bank routing | Selects CPU-visible VRAM bank `0` or `1` | Compatibility readback survives handoff; functional banking disabled | Exposed by the narrow profile without enabling native tile attributes |
| `SVBK` / `FF70` | memory/bus WRAM bank routing | Selects WRAM bank for `D000-DFFF`, with selector `0` mapping bank `1` | Unavailable `$FF` | Exposed by the narrow profile |
| `BCPS`/`BCPD`, `OCPS`/`OCPD` | PPU palette RAM | Index/data ports for BG/OBJ RGB555 palette RAM, with PPU Mode `3` blocking handled by [`PPU.md`](PPU.md) | Boot-visible index readbacks and compatibility palette seed; data mutation disabled | Index latches/readbacks only; data ports unavailable |
| `OPRI` / `FF6C` | PPU priority policy | Implemented latch/readback and boot-selected priority mode; ordinary post-boot writes do not currently mutate visual priority | Unavailable | Latch/readback only; no live visual priority mutation |
| `HDMA1`-`HDMA5` / `FF51`-`FF55` | DMA controller | GDMA and HDMA route through [`DMA.md`](DMA.md) | Unavailable | Unavailable |
| `RP` / `FF56` | bus infrared sensor + link topology | Emitter, read-enable, sensor-light, warmup/fade, and save-state state | Unavailable | Enabled by the narrow profile |
| `PCM12` / `FF76`, `PCM34` / `FF77` | APU digital taps | Read-only channel-output nibbles before DAC | Read-only CGB-family taps | Read-only CGB-family taps |
| `FF72`-`FF75` | CGB miscellaneous system state | `FF72`-`FF74` read/write bytes; `FF75` exposes writable bits `4..=6` over forced `$8F` | Boot-HWIO-visible subset only: `FF72=$00`, `FF73=$00`, `FF75=$8F`, while `FF74` remains unavailable | Exposed by the narrow profile |

## Speed and timing contract

- `SpeedController` owns `KEY1`, current speed, prepare-switch state, and save-state continuation.
- A prepared CGB `STOP` toggles speed, clears the prepare bit, resets the shared divider through the speed-aware path, and enters the modeled speed-switch pause; CPU-visible bus traffic is absent during the pause while the LCD scan domain continues under its own cadence.
- Normal and double speed both advance the CPU-visible scheduler timeline, but LCD and APU work are gated to the undoubled domain in double speed. Timer, APU frame-sequencer, serial, DMA, and PPU consumers must use the shared speed-domain helpers instead of inventing local double-speed formulas.
- The LCD/PPU dot model remains a scan-domain contract. Double speed must not be implemented as “twice as many dots per frame” or as a generic alteration of `LY`, `STAT`, or frame length.
- RTC wall-clock progression remains cartridge-owned and must not inherit the CPU speed multiplier; the MBC3 contract is documented in [`CARTRIDGES-MBC.md`](CARTRIDGES-MBC.md).

## Memory, bus, and DMA contract

- CGB VRAM is two banks. CPU-visible bank selection is controlled by `VBK`, while PPU fetches use latched tile attributes and must not be retargeted by later CPU `VBK` writes for already-fetched pixels.
- CGB WRAM keeps bank `0` fixed at `C000-CFFF` and maps `D000-DFFF` through `SVBK`, with effective selector `0` mapping bank `1`. Echo RAM follows the same resolved WRAM backing.
- CGB OAM DMA, GDMA, and HDMA share the DMA controller. CGB-specific bus impact, HBlank advance, source/destination masking, cancellation, and double-speed duration are owned by [`DMA.md`](DMA.md), not by ad hoc CGB bus branches.
- CGB-only MMIO descriptors must carry both nominal availability and current implementation state so unavailable registers do not become accidental RAM.
- CGB-only cartridge behavior is still cartridge-owned. MBC30, MBC6, MBC7, RTC, rumble, sensors, EEPROM, and future special cartridges must stay behind the typed cartridge device contract in [`CARTRIDGES-MBC.md`](CARTRIDGES-MBC.md).

## PPU and color contract

- Native CGB rendering uses RGB555 palette RAM and a raw logical RGB555 framebuffer as the primary visual output. The legacy shade framebuffer remains a DMG/debugging surface, not a native CGB presentation path.
- Native CGB BG/window fetches latch tile-map attributes from VRAM bank `1` with palette index, VRAM tile-bank, flips, and BG-priority sideband. Attribute writes affect later fetches only after the documented latch boundary.
- Native CGB OBJ fetches consume OAM attributes, OBJ VRAM bank selection, flips, OBJ palette index, and OBJ priority sideband; OBJ color index `0` remains transparent before any palette lookup.
- Native CGB priority composition follows CGB BG-over-OBJ rules, boot-selected OBJ priority mode, BG attribute priority, OAM priority, and BG color-index `0` behavior. `OPRI` post-boot visual mutation remains deferred until evidence justifies it.
- CGB compatibility mode renders DMG software through the CGB compatibility RGB555 palette adapter selected by boot/direct-start policy. It must not reuse desktop DMG palettes and must not enable native CGB tile attributes.
- CGB-family DMG-software Mode `3` live writes may share DMG-visible register contracts while using CGB-family timing/onset seams. Keep those seams in [`PPU.md`](PPU.md) and avoid ROM-specific visual patches.

## Boot and startup contract

- CGB `RealBoot` maps firmware through the non-contiguous CGB windows `0000-00FF` and `0200-08FF`, while `0100-01FF` remains cartridge/header-visible until boot unmaps.
- CGB `RealBoot` does not preselect native or compatibility mode before firmware executes. The boot ROM writes `KEY0`; the `FF50` handoff locks the boot-written value and updates machine, bus, speed, and PPU operating-mode state together.
- `SkipBoot` and `CustomBoot` synthesize centralized post-boot CGB state from the cartridge header and startup policy. Header-aware CPU registers, timer/DIV buckets, palette seed, `KEY0` lock state, CGB memory residue, wave RAM policy, and custom-boot raster phase belong in [`BOOT-ROM.md`](BOOT-ROM.md), not in local runner overlays.
- Direct-start CGB mode selection uses the canonical parsed cartridge header and heuristic policy, not ad hoc reads of raw `0x0143` in unrelated subsystems.

## APU, serial, infrared, and link contract

- CGB APU differences extend the same APU pipeline described in [`APU.md`](APU.md): speed-domain frame-sequencer gating, model/revision gates, CGB audio register behavior, and read-only `PCM12`/`PCM34` taps must not bypass channel state or host audio boundaries.
- CGB serial high-speed mode belongs to [`SERIAL.md`](SERIAL.md). `SC.1` is functional only in native CGB and the narrow experimental profile, affects internally clocked master transfers only, and consumes shared speed-domain edge bits.
- CGB infrared `RP` state is bus-owned, while two-console optical routing and single-accessory sessions are link-owned. The detailed topology and accessory contracts live in [`LINK.md`](LINK.md) and [`../info/CGB-INFRARED.md`](../info/CGB-INFRARED.md).
- Frontends may expose CGB IR sessions, audio recording, palettes, or diagnostics, but those controls must observe or configure explicit core seams rather than changing hardware timing or MMIO semantics.

## Save-state and determinism contract

- Save states must include every CGB hidden state that affects continuation: model axes, locked `KEY0`, speed controller, bank selectors, banked WRAM/VRAM contents, palette RAM and index latches, OPRI latch/boot-selected priority mode, DMA/GDMA/HDMA state, CGB misc registers, infrared sensor state, serial high-speed state, and subsystem-specific CGB hidden phases.
- Derived read-only surfaces such as `PCM12`/`PCM34` should be recomputed from owner state rather than serialized as independent truth.
- Deterministic tests may use `SkipBoot` or `CustomBoot`, but hardware claims that depend on boot handoff must also have `RealBoot` or retained external-ROM evidence as described in [`../TESTING.md`](../TESTING.md) and [`../info/ROM-SUITES.md`](../info/ROM-SUITES.md).

## Validation

- CGB work must preserve the accepted DMG baseline while adding CGB evidence through the promoted and extra/internal CGB suite lanes documented in [`../info/ROM-SUITES.md`](../info/ROM-SUITES.md).
- Use focused Rust tests for register routing, mode gates, readbacks, save-state continuation, speed-domain scheduling, bank selection, palette ports, DMA state, APU taps, serial speed, and infrared sensor state before relying on broad visual ROMs.
- Use RGB555 framebuffer fixtures for native CGB visual acceptance and keep DMG shade/rank fixtures separate from native CGB presentation.
- The CGB roadmap inventory and promotion order live in [`../roadmap/10-cgb.md`](../roadmap/10-cgb.md). Do not copy suite tables or pass counts into this hardware contract.

## Deferred boundaries

- CGB0/CGB-A/CGB-B activation, additional CGB revision gates, and hardware-revision-specific analog tuning remain deferred until validated.
- Full PGB/PSM behavior, PSM NMI, boot-ROM remap side effects after ordinary handoff, external-LCD/PGB presentation behavior, and undocumented live `KEY0`/`OPRI` visual interactions remain out of scope for the current CGB model.
- AGB/AGS/GBP profiles are future model-axis work, not aliases for current CGB behavior.
- Additional IR accessories, HuC1/HuC3-to-CGB IR protocols, and linked serial transports require explicit topology/device ownership before being treated as supported.

## Pitfalls

- Do not treat `GbCompatible` as DMG silicon.
- Do not treat `CgbDmgExt` as native CGB or as full PGB/PSM support.
- Do not use CGB revision variants as generic behavior switches.
- Do not make CGB-only MMIO a generic byte bank.
- Do not let frontend palette, audio, timing, or IR UX mutate core hardware state implicitly.
- Do not model double speed as a global frame, dot, or host-sample-rate multiplier.
- Do not enable native tile attributes, palette data mutation, HDMA, or live `OPRI` visual changes in compatibility-oriented modes without explicit evidence.
