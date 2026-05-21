# BOOT ROM

## Scope

Own boot ROM assets, boot-ROM enable/disable state, power-up sequencing, direct-boot state definitions, and hardware revision differences visible at startup.

## Hardware model

Distinguish clearly between running through boot ROM code and starting from an already initialized state. DMG and CGB must not share assumed initial state without evidence.

Within the DMG family, do not collapse `DMG-CPU`, later `DMG-CPU A/B/C`, and `CPU MGB` firmware-visible differences into one generic startup profile if observable differences matter. The public `ConsoleModel` axis names the visible product model (`GameBoy`, `GameBoyPocket`, `GameBoyLight`, `GameBoyColor`), while `HardwareRevision` names the CPU revision profile and derives the concrete firmware image selected for `RealBoot`. For DMG-family support, prefer one shared hardware core with revision-aware direct-boot profiles and explicit boot ROM images rather than separate emulator implementations per model. Treat the CGB boot ROM as a larger and structurally different firmware flow, not as a simple DMG boot ROM variant with a few extra writes. The boot subsystem should own revision-derived firmware selection and boot-ROM enable state, while the bus consumes that state when routing accesses. Real boot should start CPU execution at `0x0000` with the selected internal boot ROM mapped over the low cartridge region, and hand off to cartridge code only after a real write to `FF50` changes the mapping. `SkipBoot` and `CustomBoot` should be separate explicit direct-start initialization paths rather than partially executed or silently shortened boot ROM flows, and neither should require a boot ROM asset.

## Responsibilities

- select between explicit real-boot, skip-boot, and custom-boot startup modes
- boot ROM enable/disable behavior
- boot ROM mapping policy and state exposed to the bus
- revision-derived boot ROM image selection for `RealBoot`
- product-specific direct-boot initial register and memory state
- revision-aware startup configuration
- DMG-family differentiation where boot ROM or startup-visible behavior differs
- configurable boot ROM source selection
- direct-boot configuration for tests and tooling
- CGB split boot ROM mapping and compatibility-mode entry rules
- boot-time selection of the CGB-family `OperatingMode` from the firmware-written `KEY0` state at `FF50` handoff

## Product and firmware profiles

`ConsoleModel` is the visible console product exposed to frontends and persisted settings. `HardwareRevision` is the CPU/revision profile for that model; it is the only public/configurable revision axis and `RealBoot` always derives the boot ROM filename, expected size, and expected SHA-256 from it. A separate boot-ROM-kind axis is not public configuration: if a small private implementation descriptor is useful internally, it must be derived from `HardwareRevision` and must not be persisted, exposed in manifests, or accepted as a frontend setting. `SkipBoot` and `CustomBoot` use the selected revision for silicon/direct-start behavior but do not load or emulate boot-ROM bytes.

| ConsoleModel | Active revisions | Default revision | RealBoot filename from default | Direct-start profile |
|---|---|---|---|---|
| `GameBoy` | `DmgCpuC` | `DmgCpuC` | `dmg_boot.bin` | standard DMG post-boot profile |
| `GameBoyPocket` | `CpuMgb` | `CpuMgb` | `mgb_boot.bin` | MGB post-boot profile |
| `GameBoyLight` | `CpuMgb` | `CpuMgb` | `mgb_boot.bin` | MGB post-boot profile with a distinct desktop display palette |
| `GameBoyColor` | `CpuCgbC`, `CpuCgbD`, `CpuCgbE` | `CpuCgbC` | `cgb_boot.bin` | CGB post-boot profile aligned to standard `cgb_boot.bin` handoff |

| HardwareRevision | RealBoot filename | Expected size | Expected SHA-256 profile |
|---|---|---:|---|
| `DmgCpu` | `dmg0_boot.bin` | `256` | DMG0 |
| `DmgCpuA` / `DmgCpuB` / `DmgCpuC` | `dmg_boot.bin` | `256` | standard DMG |
| `CpuMgb` | `mgb_boot.bin` | `256` | MGB |
| `CpuCgb` | `cgb0_boot.bin` | `2304` | CGB0 |
| `CpuCgbA` / `CpuCgbB` / `CpuCgbC` / `CpuCgbD` | `cgb_boot.bin` | `2304` | standard CGB |
| `CpuCgbE` | `cgbE_boot.bin` | `2304` | CGB-E |

The current active set is intentionally narrower than the modeled enum: DMG exposes only `DmgCpuC`, MGB/LGB expose only `CpuMgb`, and CGB exposes `CpuCgbC`, `CpuCgbD`, and `CpuCgbE`. Earlier DMG and CGB revision variants remain modeled so real-boot assets, save-state metadata, and future hardware validation do not need another rename when they become active.

## Registers / MMIO

- boot ROM mapping control
- startup-visible register state
- `FF50` boot ROM disable behavior
- CGB boot ROM overlays across `0000-00FF` and `0200-08FF`

## Bus-facing mapping baseline

- The boot subsystem should publish boot-ROM visibility to the bus as active overlay windows, not as one DMG-only "`0000-00FF` mapped" boolean.
- In the current DMG-family baseline, that published state enables only the low window at `0000-00FF`.
- CGB real boot publishes both the low window and the upper boot-ROM window at `0200-08FF` through the same contract, without changing bus-state shape.
- The bus may still decode those windows into one `BootRom` routed owner; the important constraint is that the mapping state itself remains window-oriented and model-aware.
- Bus-facing structured snapshots and bus-arbitration traces should expose those low and upper boot-overlay windows explicitly so tooling can observe the same routing state that the decode path is using.
- The boot-ROM asset contract must stay aligned with that mapping contract too: a `CGB` model must not silently reuse a DMG-family boot image just because the current functional target is still DMG-first.
- In the current repo baseline, CGB boot assets may be provided either as a compact `0x800`-byte image containing the two executable windows back-to-back, or as a sparse `0x900`-byte address-space image that keeps the visible cartridge gap at `0x0100-0x01FF`; in both forms only `0000-00FF` and `0200-08FF` are boot-overlay windows, and `0100-01FF` routes to cartridge/header bytes while boot ROM remains mapped.
- Strict RealBoot validation for the standard CGB path requires canonical `cgb_boot.bin` as a `2304`-byte sparse asset with SHA-256 `b4f2e416a35eef52cba161b159c7c8523a92594facb924b3ede0d722867c50c7`; `cgb0_boot.bin` and `cgbE_boot.bin` are recognized and hash-checked as revision-derived assets, and `CpuCgbE` selects `cgbE_boot.bin` automatically. The Phase 10 default remains the standard `CpuCgbC` / `cgb_boot.bin` profile.

## Boot mode baseline

- The project should support three explicit startup modes: `RealBoot`, `SkipBoot`, and `CustomBoot`.
- `RealBoot` must execute the selected boot ROM on the real CPU core, through the real bus, and on the shared T-cycle scheduler.
- `SkipBoot` must initialize a model-specific post-boot state directly, start execution at `0x0100`, and leave boot ROM mapping disabled from the beginning.
- `CustomBoot` must remain an explicit direct-start path rather than a partial boot-ROM execution: DMG-family custom boot initializes the same direct CPU, visible I/O, and hidden-state baseline as `SkipBoot` and overlays the DMG boot-logo tile bytes and logo tilemap, while CGB custom boot preserves the CGB direct-start CPU/memory base, applies the header-aware cartridge-entry timer/raster phase for boot-sensitive CGB fixtures, writes only the boot-logo tile bytes, and leaves the `$9904/$9924` tilemap residue clear.
- The rest of the system should not care whether execution reached cartridge code through real boot, skip boot, or custom boot; only the configured startup path should differ.
- In the current repo baseline, keep centralized direct-start contracts explicit instead of conflating them: DMG-family `Machine::SkipBoot` keeps the deterministic continuity profile used by core/fixture tests, `Machine::CustomBoot` adds the explicit DMG boot-logo tile and tilemap seed to that direct baseline, CGB `Machine::SkipBoot` derives the direct handoff from the cartridge header where that evidence is currently validated, CGB `Machine::CustomBoot` preserves the same CGB CPU/memory direct-start base while using the core cartridge-entry timer/raster phase and only the DMG logo tile bytes, and `BootController::direct_boot_state()` owns the verified DMG-family cartridge-entry snapshot used to validate real `FF50` handoff behavior.
- Replacing the loaded cartridge on an existing `Machine` must restart hardware state from that same configured startup path instead of splicing the new ROM into an already-advanced runtime. Reset the scheduler and trace timeline back to `t_cycle = 0`, rebuild CPU / bus / DMA / timer / serial / PPU / APU / interrupt / boot state from power-on or skip-boot rules, and only preserve explicitly host-owned controls such as debugger configuration, serial-peer selection, and the current effective host joypad state.
- Treat the post-boot snapshot as a mix of fixed-per-model values, cartridge-header-derived values, explicitly unreliable or uninitialized values, and hidden temporal state that must be synthesized coherently.

## DMG-family boot baseline

- DMG-family boot ROM selection should remain explicit through `HardwareRevision` values covering at least `DmgCpu`, `DmgCpuA/B/C`, and `CpuMgb`, with the concrete filename derived rather than independently configured.
- Those DMG-family boot ROMs should run on the same DMG hardware core without scattered model-specific CPU branches.
- During real DMG-family boot, the boot ROM should read the cartridge logo/header bytes, perform its documented checks, drive the visible animation through ordinary CPU and bus activity, and withhold cartridge handoff when those checks fail.
- The visible boot logo should come from the cartridge header bytes at `0x0104-0x0133`, not from a frontend animation script or emulator-side asset.
- The "no cartridge / reads as `0xFF`" boot behavior should emerge from cartridge and bus behavior rather than a special visual hack.
- While boot ROM is mapped, it should overlay the relevant low cartridge range through ordinary bus routing; once unmapped, the cartridge entry point and header at `0x0100-0x014F` should be visible again through normal cartridge decode.
- Real-boot header checks must use ordinary cartridge reads on the shared T-cycle timeline; they must not bypass the cartridge device with a second header-copy path.
- Skip-boot and future model-selection logic may consume parsed cartridge metadata, but that metadata should come from the cartridge subsystem's canonical header parser rather than from a second boot-local parser.

## `FF50` handoff baseline

- `FF50` must behave as a real boot-ROM mapping-control register.
- Real boot completion should happen because boot-ROM code executes a real write to `FF50`, not because the emulator detects a conceptual "boot is done" state.
- The mapping change caused by `FF50` must affect the next fetch, not previous accesses retroactively.
- On DMG-family real boot, the first opcode fetched from the cartridge after handoff should be the byte at `0x0100`.
- That post-`FF50` fetch at `0x0100` should come from the same loaded cartridge device that already owns the header bytes and later mapper behavior.
- Register state visible at cartridge entry must come from the executed boot ROM of the selected model; do not hard-code DMG and MGB as sharing one identical final `A` value.
- `FF50` should stay a write-only MMIO control path from the hardware-contract perspective, even if the implementation keeps internal mapping state for debugging or introspection.
- The write to `FF50` should perform the mapping side effect at the access itself; do not treat it as a passive stored byte that another subsystem polls later.
- On CGB-family real boot, the `FF50` write must report an explicit newly-unmapped edge so the machine can lock the current boot-written `KEY0` state exactly at the handoff boundary.
- CGB `RealBoot` must not preselect native CGB versus DMG-compatibility mode from cartridge metadata before executed firmware reaches `FF50`; the boot ROM writes `KEY0`, then the `FF50` handoff locks that state and updates `MachineConfig`, bus, speed, and PPU operating mode together.
- After the CGB handoff lock, `KEY0` remains boot-owned: ordinary software reads get the unavailable `$FF` readback and post-lock writes are ignored, while the internal locked value remains available only to boot/mode state.
- Any CGB `RealBoot` handoff phase correction must be header-bucket-specific and applied at the real `FF50` edge in core-owned timer/PPU state, not as a test-runner startup overlay, so `gb-cli`, `gb-desktop`, and ROM-suite validation see the same cartridge-entry contract.

## DMG-family skip-boot CPU snapshot baseline

- `SkipBoot` should expose CPU state matching the post-boot handoff point at `PC = 0x0100`.
- DMG `SkipBoot` CPU state should initialize `A=0x01`, `B=0x00`, `C=0x13`, `D=0x00`, `E=0xD8`, `H=0x01`, `L=0x4D`, `SP=0xFFFE`, and `PC=0x0100`.
- MGB `SkipBoot` CPU state should match DMG except for `A=0xFF`.
- DMG0 `SkipBoot` CPU state should use its own table rather than reusing later DMG defaults: `A=0x01`, `B=0xFF`, `C=0x13`, `D=0x00`, `E=0xC1`, `H=0x84`, `L=0x03`, `SP=0xFFFE`, `PC=0x0100`.
- For DMG and MGB, `F` should not be a single hard-coded constant; `Z=1` and `N=0` remain fixed, while `H` and `C` should derive from the cartridge header checksum (`0x00` leaves both cleared, any other checksum leaves both set).
- DMG0 `F` should start cleared in the direct post-boot snapshot.
- Cartridge-header-dependent post-boot CPU state should be derived from the loaded cartridge data rather than duplicated as disconnected literals.

## DMG-family direct-start snapshot baseline

- Keep DMG-family direct-start I/O snapshots centralized and model-aware rather than scattering startup literals across subsystems.
- The deterministic DMG-family `Machine::SkipBoot` continuity snapshot currently uses `P1=0xCF`, `SB=0x00`, `SC=0x7E`, `DIV=0xAB`, `TIMA=0x00`, `TMA=0x00`, `TAC=0xF8`, `IF=0xE1`, `LCDC=0x91`, `STAT=0x85`, `SCY=0x00`, `SCX=0x00`, `LY=0x00`, `LYC=0x00`, `DMA=0xFF`, `BGP=0xFC`, `WY=0x00`, `WX=0x00`, and `IE=0x00`.
- The DMG-family `Machine::CustomBoot` continuity snapshot uses the same CPU, visible I/O, and hidden subsystem state as `Machine::SkipBoot`, while the startup memory policy writes the DMG boot-logo tile stream at even VRAM addresses starting at `$8010` and the logo tilemap at `$9904..$992F`, including the second tilemap row at `$9924..$992F`; this seed is a test/tooling direct-start contract and not evidence that normal SkipBoot memory should contain the logo. The CGB `Machine::CustomBoot` memory policy keeps the CGB direct-start WRAM/HRAM/VRAM base and writes the same logo tile stream without the `$9904..$992F` tilemap overlay, matching the CGB no-boot fixture distinction between logo tiles and clear tilemap residue.
- The deterministic CGB `Machine::SkipBoot` continuity snapshot is aligned to standard `cgb_boot.bin` handoff for the fields owned by Slice 6: CGB native headers enter with `AF=$1180`, `BC=$0000`, `DE=$FF56`, `HL=$000D`, `SP=$FFFE`, and `PC=$0100`, while DMG-only compatibility headers enter with `AF=$1180`, `BC=$0000`, `DE=$0008`, `HL=$007C`, `SP=$FFFE`, and `PC=$0100`; both CGB paths seed released joypad state with unselected rows so the visible `P1` handoff readback is `$FF`.
- The same CGB `Machine::SkipBoot` path keeps the CGB DIV/timer state coherent through a header-aware direct-start predictor: missing cartridges and DMG-compatible headers keep the Slice 2 CGB ABCDE baseline (`DIV=$26` with hidden timer counter `$2674`) validated by Mooneye `misc/boot_div-cgbABCDE.gb`, the currently validated native CGB non-Nintendo old-licensee bucket seeds `DIV=$1E` with hidden timer counter `$1E84` for Hacktix `bully.gb`, and the native CGB old-licensee `$33` plus binary-zero new-licensee bucket seeds `DIV=$1E` with hidden timer counter `$1E98` for Nitro2k01 `whichboot.gb`. The same path also mirrors the boot-owned RealBoot handoff state that is deterministic in this repo: locked `KEY0`-derived `OperatingMode`, `KEY1`/bank/MMIO readbacks for the selected mode, CGB compatibility-mode `BCPS=$C8` and `OCPS=$D0` index readbacks, CGB palette RAM seed and `OPRI` state where exposed by the PPU model, alternating CGB wave RAM bytes, and boot-owned visible memory prefixes in VRAM/WRAM/HRAM.
- The verified DMG-family cartridge-entry snapshot owned by `BootController::direct_boot_state()` now tracks the real-handoff-visible fields used by the repo-local regression matrix: `P1=0xCF`, `DIV=0xAB`, `STAT=0x81`, and `LY=153` for `DMG` / `MGB`; and the `DMG0` direct-entry snapshot remains separate from the later-DMG RealBoot lane until that firmware path is revalidated.
- The direct post-boot snapshot should also include the published DMG-family audio-register values rather than leaving the APU block in a made-up default state.
- If direct boot does not have verified firmware-derived wave RAM contents, it should keep wave RAM under an explicit startup policy rather than presenting a fake published constant.
- Mixed-register snapshots such as `P1`, `SC`, `TAC`, `IF`, `STAT`, `DIV`, and `NR52` should be realized through subsystem-owned latched/live or forced-bit state synthesis rather than through blind raw-byte stores that would contradict their MMIO contract.
- Serial startup values such as `SB` and `SC` should likewise be applied through serial-owned control and idle-transfer state rather than through a fake generic I/O byte array.
- `OBP0` and `OBP1` should not be treated as reliable fixed hardware constants in `SkipBoot`; keep them under an explicit emulator policy for uninitialized state instead of inventing a false canonical value.
- Values that remain undefined or unreliable after power-up should stay explicitly classified as such even when `SkipBoot` is used.

## Hidden-state synthesis baseline

- Both the deterministic `Machine::SkipBoot` path and the verified direct-boot snapshot used for real-boot validation must synthesize internal subsystem state, not only visible registers.
- Joypad state should be initialized coherently with the visible `P1` snapshot, including row-selection lines and released-button state, rather than by treating `P1` as a flat stored byte.
- The timer's internal counter and overflow-related state should be initialized coherently with the visible `DIV`, `TIMA`, `TMA`, and `TAC` snapshot at `PC = 0x0100`.
- For the current deterministic `Machine::SkipBoot` continuity baseline, that coherence includes one explicit hidden divider phase instead of only the visible `DIV` byte. The current model seeds the shared timer system counter to `0xABC8` for DMG-family direct boot so Mooneye's DMG-family `boot_div` cadence lines up with the expected first post-boot `DIV` edges. CGB direct boot is header-aware for validated buckets: missing and DMG-compatible headers keep the `0x2674` CGB ABCDE baseline used by Mooneye `misc/boot_div-cgbABCDE.gb`, native CGB non-Nintendo old-licensee headers currently use `0x1E84` so Hacktix `bully.gb` sees the same initial-`DIV` family as gb-cycle's standard `cgb_boot.bin` RealBoot handoff, and native CGB old-licensee `$33` headers with binary-zero new-licensee bytes use `0x1E98` so Nitro2k01 `whichboot.gb` sees the hardware-facing GBC entry fingerprint without runner-only timer overlays.
- The verified later-DMG / MGB cartridge-entry snapshot used by the real-boot regression keeps its own visible PPU/APU continuation coherent with observed handoff values, while the joypad `P1` row-select lines start asserted (`selection_bits=0x00`) so the visible handoff value is `0xCF` with no host buttons pressed.
- The later-DMG / MGB `RealBoot` power-on path seeds the hidden timer counter with its own `0x0064` reset offset so executing the standard DMG-family boot ROM reaches the `whichboot.gb` hardware fingerprint `LY:00 DIV:AB.34` at cartridge-side capture; keep that power-on phase independent from the deterministic `Machine::SkipBoot` hidden-divider phase.
- The same `RealBoot` power-on state seeds the DMA-owned `FF46` source-page latch to `0xFF`, matching `gbmicrotest/boot/poweron_dma_000.gb`; keep this as DMA-owned latched state rather than a generic I/O byte.
- The standard CGB `RealBoot` power-on path seeds the hidden timer counter to `0xFFFB` so executing canonical `cgb_boot.bin` preserves the Mooneye `boot_div-cgbABCDE` DMG-compatible `0x2674` bucket and the Hacktix `bully.gb` native CGB non-Nintendo `0x1E84` bucket; the native CGB old-licensee `$33` plus binary-zero new-licensee bucket used by Nitro2k01 `whichboot.gb` applies a narrow `24` T-cycle handoff correction at the real `FF50` edge, advancing timer and CGB raster together to the validated cartridge-entry phase (`DIV=$1E` hidden counter `$1E98`, `LY=$90`, line dot `173`) without changing the global CGB power-on seed.
- Serial state should be initialized coherently with visible `SB` and `SC`, including idle transfer state, clock mode, no in-flight shift progress, and its own hidden free-running master-clock phase rather than by treating serial as two disconnected plain bytes.
- For the current deterministic `Machine::SkipBoot` continuity baseline in this repo, that serial hidden phase is seeded independently to `0xABCC` at `PC = 0x0100`; the later-DMG / MGB `RealBoot` power-on path seeds the serial free-running counter with its own `0x0068` reset offset so the same `0xABCC` phase is reached at the verified `FF50` handoff. It must not be synthesized by reusing the timer's `0xABC8` divider phase because Mooneye's `boot_sclk_align` timing depends on that separation.
- The PPU's internal mode, dot position, and related pipeline state should be initialized coherently with the visible `LCDC`, `STAT`, `LY`, `LYC`, and other LCD-facing registers at `PC = 0x0100`.
- For the current deterministic DMG-family `Machine::SkipBoot` continuity baseline, that PPU coherence includes a first-frame hidden Mode `0` STAT IRQ phase in addition to the visible `LY=0` / `STAT=$85` snapshot, so startup HBlank interrupt tests can distinguish readable `IF`, same-cycle interrupt wake, and the ordinary post-startup STAT edge.
- For the same deterministic DMG-family `Machine::SkipBoot` continuity baseline, PPU coherence also includes a boot-facing CPU-bus publication overlay for the earliest cartridge-entry probes: `FF44`, `FF41`, OAM accessibility, and VRAM accessibility follow the `gbmicrotest poweron_*` delay table while the internal raster remains synthetic `LY=0`; this hidden startup phase must stay separate from the verified `BootController::direct_boot_state()` `LY=153` snapshot.
- For the current later-DMG `RealBoot` path, executing the boot ROM seeds the first LCD enable with an observed hidden dot phase of `92` dots and, at the real `FF50` handoff, arms both a handoff-frame Mode `0` STAT seam for `SCX&7 == 3` / `SCX&7 == 7` and a handoff-relative boot-facing `gbmicrotest poweron_*` CPU-bus publication overlay; keep those as explicit PPU hidden state instead of changing the visible `FF50` mapping contract or reusing the `SkipBoot` frame-origin base.
- The APU's internal `DIV-APU` / frame-sequencer phase and other timing-visible audio state should ultimately be initialized coherently with the visible post-boot `NRxx` snapshot at `PC = 0x0100`.
- In the current repo baseline, direct boot only guarantees the APU state that is already represented in the centralized startup snapshot and the APU-owned reconstruction path: visible `NRxx` ownership, powered state, wave-RAM startup policy, channel-active reconstruction from `NR52`, and `DIV-APU` seeded from the chosen shared-divider preset.
- Remaining hidden audio state such as HPF capacitor/history, pulse duty-step continuation, CH3 sample-buffer/sample-index continuation, and CH4 LFSR/noise-timer continuation is still reconstructed from repo-local defaults rather than from verified boot-handoff evidence; document that gap explicitly instead of calling the whole APU startup path coherent today.
- The deterministic `Machine::SkipBoot` path must therefore avoid impossible hidden-state discontinuities in the subsystems it already seeds explicitly, while the remaining APU hidden-state gaps stay recorded as deferred work for later hardening and oracle validation.

## Uninitialized and cartridge-dependent startup state

- `SkipBoot` must distinguish between values that are fixed by model, values derived from the cartridge header, and values that are genuinely unreliable after power-up.
- Pan Docs and hardware research do not support treating WRAM and HRAM as fixed zero-filled memory in the direct post-boot snapshot; they remain unreliable or effectively random across power-up.
- Cartridge RAM, whether external or mapper-local to the cartridge controller, should not be assumed clean on first power-up when a direct post-boot path is used.
- A direct-boot path should use an explicit policy for uninitialized memory and unreliable registers, such as seeded pseudo-random data, a documented pattern, or a debug-oriented deterministic startup policy.
- In the current repo baseline, DMG-family `SkipBoot` uses an explicit deterministic patterned policy for WRAM and HRAM so continuity tests stay reproducible without claiming those bytes are hardware constants, while DMG-family `CustomBoot` keeps that WRAM/HRAM policy and overlays the DMG boot-logo tile and tilemap bytes. CGB `SkipBoot` uses the explicit `cgb_boot.bin` handoff policy for deterministic comparison against RealBoot: zeroed WRAM, zeroed HRAM with the boot logo prefix, and zeroed VRAM with the boot logo tile prefix; CGB `CustomBoot` preserves that CGB handoff memory base and overlays only the DMG boot-logo tile bytes, leaving the logo tilemap clear for CGB framebuffer oracles that do not inspect DMG tilemap residue.
- That uninitialized-state policy must not overwrite values that are deterministic in the documented post-boot snapshot.

## Post-boot visible map baseline

- `SkipBoot` and `CustomBoot` should begin with boot ROM already unmapped and normal cartridge visibility restored.
- After `SkipBoot` or `CustomBoot`, the ordinary cartridge ROM map across `0x0000-0x7FFF` should be visible again: the fixed low ROM region should no longer be covered by boot ROM, and the switchable cartridge-ROM region should remain under normal mapper control where applicable.
- After handoff, the same cartridge device should expose `0x0100-0x014F` for entry-point execution, header inspection, and later mapper-owned reads without a boot-specific shadow copy.
- `FF50` should still exist as the boot-ROM mapping-control register even though direct-start modes start with boot ROM already disabled.
- DMG-mode reads from CGB-only registers that do not exist functionally yet should return `0xFF` rather than emulator-invented values.

## Timing / accuracy requirements

- Boot ROM transition behavior must remain explicit.
- Direct-boot helpers must document what state they assume and why.
- The boot ROM should execute as real CPU code from `0000-00FF` while mapped.
- Unmapping of the boot ROM and handoff to cartridge ROM must happen through the documented hardware-visible mechanism, not through an implicit emulator shortcut.
- CGB support must account for the fact that the boot ROM mapping is not the same shape as DMG-family boot ROM mapping: only `0000-00FF` and `0200-08FF` are boot overlays, and the `0100-01FF` gap remains cartridge-visible while boot remains mapped.
- Real boot must use the same CPU core, bus, and shared T-cycle scheduler as the rest of emulation rather than a special startup interpreter or frontend animation path.
- Boot ROM reads from the cartridge header and boot-ROM writes to VRAM/LCD/MMIO should use the same bus and arbitration rules as ordinary execution.
- The duration of the boot process should emerge from executed instructions and subsystem timing, not from an external startup timer.
- Direct-start modes must remain distinct initialization paths; do not partially execute the boot ROM and cut it short.
- The repo should keep the deterministic synthetic `Machine::SkipBoot` startup state and the verified direct-boot cartridge-entry snapshot explicit and separately documented instead of letting one silently stand in for the other.
- A direct-boot preset must not stop at visible MMIO values alone; it should also establish hidden timer, PPU, and APU state consistent with those values.
- Direct-boot initialization of indeterminate memory and unreliable registers should remain explicit and configurable rather than pretending the hardware guarantees a single power-up value.

## Dependencies

- CPU
- T-cycle scheduler or clock source
- bus and memory
- cartridge/MBC
- timer
- PPU
- APU
- model/revision configuration

## Primary references

- Pan Docs boot process sections
- Gekkio hardware documentation and revision material
- Mooneye documentation and tests

## Open-source emulator references

Priority order:

1. SameBoy
2. Mooneye GB
3. binjgb
4. GameRoy
5. Gambatte

## Tests

- real-boot versus skip-boot entry-path tests
- boot ROM presence/disable tests
- model-specific startup state tests
- tests for `FF50` handoff from boot ROM to cartridge ROM
- tests that `0x0000-0x00FF` read from boot ROM before `FF50` and from cartridge ROM after `FF50`
- tests that the next fetch after `FF50` already comes from the cartridge and that DMG-family real boot enters cartridge code at `0x0100`
- tests for valid header/logo/checksum handoff versus invalid-logo or invalid-checksum no-handoff behavior
- tests for missing-cartridge or `0xFF`-filled header behavior
- tests for strict boot ROM size plus SHA verification before RealBoot execution
- tests for CGB boot-overlay windows at `0000-00FF` and `0200-08FF`, including compact `0x800` and sparse `0x900` images, and tests that `0100-01FF` routes to cartridge/header bytes while boot ROM is still mapped
- tests for CGB `KEY0` writes before handoff, lock-on-`FF50`, post-lock read/write behavior, native versus compatibility operating-mode propagation, and save/restore of the locked handoff state
- ignored local tests that compare CGB RealBoot handoff snapshots against centralized CGB `SkipBoot` for valid native and compatibility headers and keep invalid checked-logo, checksum, and all-`FF` header cases on the no-handoff path
- tests for model-specific visible `A` at cartridge entry, especially DMG versus MGB
- direct-boot preset tests that document assumed register state
- direct-boot CPU-register tests for `DMG0`, DMG with checksum `0x00`, DMG with checksum not `0x00`, and MGB
- direct-boot I/O readback tests for the published post-boot snapshot
- direct-boot tests that verify the ordinary cartridge ROM map is visible again after startup, including `0x0000`, `0x0100`, and mapper-controlled regions where applicable
- continuity tests for the first T-cycles after `SkipBoot`, especially around timer, PPU, and APU state derived from the visible post-boot registers
- tests that document the chosen policy for WRAM, HRAM, cartridge RAM whether external or mapper-local, `OBP0`, and `OBP1` when direct boot bypasses firmware execution

## Implementation notes for this repo

- Keep "after boot" presets separate from real boot ROM execution paths.
- Leave extension points for hardware revisions and variants.
- The first core may target the DMG family, but the boot path must still depend on an explicit console model enum or equivalent typed descriptor.
- Boot ROM loading should be configurable so the emulator can use real dumps, custom firmware, or no boot ROM at all.
- A dedicated `BootRom` component with bytes, selected kind, and mapped/unmapped state is the intended baseline.
- Keep boot-ROM asset ownership and boot enable/disable state in the boot subsystem even if the bus performs the actual address routing.
- Keep boot-ROM asset selection model-aware too: DMG-family models may keep their `0x100`-byte images, but `ConsoleModel::GameBoyColor` should select an explicit `CGB` boot image kind rather than aliasing to `DMG`.
- Keep real-boot, skip-boot, and custom-boot as explicit modes such as `RealBoot`, `SkipBoot`, and `CustomBoot`; the rest of the emulator should see only the resulting machine state and bus mapping.
- A `SkipBoot`, `CustomBoot`, or equivalent explicit direct-boot mode is useful for tests, tooling, and differential validation, but it must remain distinct from verified boot ROM execution.
- DMG-family observable differences should initially be assumed to come from firmware and startup state unless a proven hardware-level difference matters to the emulator.
- `FF50` should integrate with system or bus mapping control, not as a CPU-local shortcut.
- Real-boot header validation should emerge from executed boot-ROM code reading cartridge bytes, not from a parallel emulator-side validator.
- The synthetic DMG boot ROM from Phase `2.4` remains a useful narrow integration target inside `gb-core`, but production DMG-family boot closure in this repo is defined by ignored regressions that run verified `dmg0` / `dmg` / `mgb` boot ROM assets supplied through `GB_CYCLE_BOOT_ROM_ROOT` through the real CPU, bus, scheduler, and `FF50` handoff path while keeping invalid DMG logo/checksum/header cases on the non-handoff side.
- Boot should consume cartridge-derived metadata such as checksum-dependent post-boot flags, `cgb_flag`, or `sgb_flag` through the cartridge subsystem's canonical parsed header view rather than by reparsing header bytes in multiple places.
- A central routine such as `initialize_post_boot_state(model, cartridge)` is the preferred shape for direct-start modes, with one source of truth for model-specific CPU state, visible I/O state, and hidden-state synthesis inputs.
- Keep direct-boot snapshot data centralized in typed structures rather than copying startup literals into CPU, timer, PPU, APU, or bus modules independently.
- That centralized post-boot snapshot should own initial visible values only; each subsystem must still own the live semantics of its registers after startup.
- Cartridge-derived post-boot fields such as DMG/MGB `F` should be computed from the loaded header at initialization time rather than hard-coded into one static table.
- Uninitialized-state policy for WRAM, HRAM, cartridge RAM whether external or mapper-local, `OBP0`, and `OBP1` should be explicit and testable.
- Do not hard-code boot ROM support around a fixed 256-byte assumption; CGB boot ROM is larger and uses a split mapped layout.
- Keep the bus-facing boot mapping state aligned with that split-layout requirement: the contract should stay able to express multiple active windows even while the current functional target is still DMG-only.
- For CGB `RealBoot`, boot should let executed firmware inspect cartridge header compatibility information and choose CGB mode or DMG-compatibility mode through `KEY0`; for CGB direct-start modes, the centralized startup state may derive the equivalent locked mode from the canonical cartridge header parser so tests can start at the same post-handoff contract without running firmware.
- In the current repo baseline, the boot trace should make the `FF50` mapping state visible on the same timeline as the preceding CPU write and the following cartridge fetch, so handoff ordering can be debugged without a separate boot-only trace path.

## Known pitfalls

- assuming DMG and CGB initial state are interchangeable
- mixing convenience startup state with verified boot behavior
- hard-coding one boot ROM path into the core
- forcing real boot to jump to `0x0100` without a real `FF50` write and next-fetch handoff
- validating logo or checksum outside the executed boot ROM path
- faking the Nintendo logo or boot animation in a frontend layer instead of letting VRAM/LCD writes emerge from execution
- silently jumping to post-boot state without making the mode explicit
- treating `SkipBoot` as "set a few famous CPU registers" while leaving timer, PPU, or APU hidden state incoherent with the published post-boot snapshot
- zero-filling WRAM, HRAM, or cartridge RAM and presenting that as documented hardware behavior
- inventing fixed post-boot values for unreliable registers such as `OBP0` and `OBP1`

## Open questions

- which models and revisions to support in the first direct-boot API
- which DMG-family differences are treated as required from day one versus deferred behind documented limitations
- how the boot mapping abstraction should represent non-contiguous firmware windows cleanly once CGB support begins
- which explicit uninitialized-state policy should be the default for direct boot in tests versus interactive use
