# CGB

## Scope

Own Color Game Boy-specific behavior: double speed, VRAM banks, WRAM banks, palettes, HDMA, and other model-specific extensions beyond shared DMG behavior.

## Hardware model

Design interfaces so CGB behavior remains an extension of the shared core rather than a parallel emulator. Separate DMG-only, shared, and CGB-only behavior explicitly.

CGB should extend the shared core through model-aware behavior and capabilities, not by introducing a parallel emulator architecture. Avoid premature CGB revision complexity in DMG-family subsystems, but keep the extension seams that prevent large future refactors.

`ConsoleModel` is the visible product model, not the CPU revision string and not the boot ROM filename. The CGB product row is `ConsoleModel::GameBoyColor`; it defaults to `HardwareRevision::CpuCgbC`, exposes active revisions `CpuCgbC`, `CpuCgbD`, and `CpuCgbE`, derives `cgb_boot.bin` or `cgbE_boot.bin` automatically for `RealBoot`, and uses a centralized CGB `SkipBoot` startup profile aligned to the standard `cgb_boot.bin` handoff for Slice 6-owned deterministic state, including header-aware direct-start timer buckets where currently validated. CPU revisions such as `CPU CGB`, `CPU CGB A`, `CPU CGB B`, `CPU CGB C`, `CPU CGB D`, and `CPU CGB E` are modeled by `HardwareRevision`, but behavior must still branch on revision only after a tested revision-specific difference is intentionally modeled. `gb-cli --revision`, `gb-desktop --revision`, `CONFIG -> SYSTEM -> REV`, and runner manifest `revision` select the same core axis so runner manifests are not the only way to reproduce revision-specific behavior. The first active gate is the CH1 CGB-E sweep-restart hold required by SameSuite/DocBoy `channel_1_sweep_restart_2-cgbE`, while unrelated CGB behavior must not branch on the revision until a concrete oracle requires it.

The cross-family model-profile table that aligns `ConsoleModel`, `HostPlatform`, CPU revision names, boot ROM filenames, operation mode, and display/color profile lives in [`core/MODEL-AXES.md`](../core/MODEL-AXES.md#reference-model-profiles). This CGB handbook owns the CGB behavior that consumes those axes, not the global product taxonomy.

The desktop presentation palettes for `GameBoy`, `GameBoyPocket`, and `GameBoyLight` intentionally live in `gb-desktop`, not `gb-core`: DMG-family desktop rendering and screenshots map shade/rank framebuffer data through SameBoy's `Core/display.c` DMG, MGB, and GBL palettes. Shade mapping follows SameBoy: shade `0` uses palette entry `3`, shade `1` uses entry `2`, shade `2` uses entry `1`, and shade `3` uses entry `0`. `GameBoyColor` presentation must bypass the Video → Palette selector and render the core CGB RGB555 framebuffer directly, including CGB compatibility mode; the desktop palette selector is therefore disabled for the CGB model.

## Responsibilities

- double-speed behavior
- banked VRAM and WRAM behavior
- color palettes
- CGB-only DMA/HDMA features
- model capability flags and feature gates
- CGB boot-time mode selection versus DMG-compatibility mode

## Implementation priority

The base CGB path is implemented in this functional order before worrying about hardware revision variants:

- CPU double speed
- two VRAM banks
- banked WRAM
- CGB palette state
- additional CGB-only I/O registers

## Registers / MMIO

- CGB palette registers
- VRAM/WRAM bank registers
- speed switch control
- HDMA registers
- boot-time interpretation of cartridge CGB compatibility flags
- `KEY0`
- `VBK`
- `SVBK`
- `BCPS`, `BCPD`
- `OCPS`, `OCPD`
- `KEY1`
- `RP`
- `PCM12`
- `PCM34`
- undocumented `FF72`, `FF73`, `FF74`, `FF75`

## DMG fallback policy for CGB-only MMIO

- The shared MMIO map should record, per register, both nominal availability and current implementation state.
- Nominal availability should distinguish shared, DMG-compatible, and CGB-only registers.
- Current implementation state should distinguish implemented, stubbed, and unavailable registers without forcing the bus to fake unsupported CGB behavior.
- In DMG mode, CGB-only registers that are not functionally implemented should return the documented non-CGB fallback value, typically `0xFF`, rather than behaving as RAM.
- Writes to those registers in DMG mode should follow an explicit ignored-or-DMG-semantics policy and must not mutate nonexistent state accidentally.
- New CGB registers and features should extend the same routed MMIO contract rather than replacing a temporary DMG-only shortcut.

## Timing / accuracy requirements

- Avoid DMG shortcuts that would break banks, palettes, HDMA, or double speed.
- Keep CGB timing and shared timing differences visible.
- Keep the timing model ready for CPU-speed changes without redefining the LCD-side temporal foundation.
- Keep DMG-family quirks such as the OAM corruption bug explicitly model-gated; CGB-family hardware must not inherit them accidentally in DMG-compatibility mode.

## Dependencies

- CPU
- PPU
- DMA
- timer
- bus and memory
- model/revision configuration

## Primary references

- Pan Docs CGB sections
- Gekkio references
- model-specific hardware research

## Open-source emulator references

Priority order:

1. SameBoy
2. GameRoy
3. binjgb
4. Gambatte
5. Mooneye GB

## Tests

- cgb-acid2
- CGB Mooneye tests
- palette/banking/HDMA focused tests
- negative tests that DMG-family OAM corruption behavior does not appear on CGB-family hardware even while running monochrome software

## Implementation notes for this repo

- Model capabilities should be centralized, not spread as random conditionals.
- The public model surface should distinguish at least `ConsoleModel`, `OperatingMode`, and `HostPlatform` so CGB silicon, CGB native mode, CGB DMG-compatibility mode, and future SGB hosting are not conflated.
- Shared subsystems should expose clean extension points for CGB-only behavior.
- DMG-family behavior should remain the baseline shared path where possible, with CGB-specific features layered on through explicit model capabilities.
- `ConsoleModel::GameBoyColor` plus `OperatingMode::GbCompatible` should mean "CGB-family silicon running monochrome software-visible mode", not "pretend this machine is a DMG".
- `GbCompatible` and `CgbDmgExt` PPU Mode `3` live-write seams should be modeled as CGB-family DMG-software behavior: they ignore native CGB BG attributes but may still need CGB-specific startup phase/onset tables, as with `LCDC.0`, `LCDC.1`, `LCDC.3`, and `LCDC.4`, and must stay separate from native-only CGB glitches such as same-cycle tile-number substitution.
- CGB support should keep architecture seams explicit for banked memory, palette state, extra I/O, HDMA, and speed switching, rather than hiding those behaviors behind generic DMG fallbacks.
- Do not claim functional closure for CGB-only special cartridges before they have dedicated validation and runtime. `MBC30` is now implemented after the base CGB gate as an explicit `MBC3`-family variant, `MBC6` is now implemented through a dedicated cartridge-local runtime and persistence model, and `MBC7` now has a dedicated sensor / EEPROM runtime path with focused validation and an explicit no-runtime-rumble policy.
- The shared CPU execution model is based on in-flight fetch/read/write/internal steps so double-speed behavior can scale the same engine instead of replacing an opcode-duration-based core.
- CPU `STOP` is represented separately from `HALT`, so CGB speed-switch behavior attaches to an explicit control state rather than a separate CPU-state redesign.
- The boot subsystem and bus treat boot-ROM mapping as model-aware routing state: CGB real boot overlays only `0000-00FF` and `0200-08FF`, while cartridge/header bytes remain visible through the `0100-01FF` gap while boot is still mapped.
- The OAM DMA implementation lives inside a reusable DMA subsystem contract so CGB OAM DMA timing differences, GDMA, and HDMA extend the same infrastructure.
- Phase 10 row promotion for `ashiepaws/bully.gb (GBC)` starts consuming the CGB-family OAM-DMA CPU-bus policy: external-source OAM DMA conflicts with cartridge ROM/RAM fetches and OAM destination access, but it must not block internal WRAM, HRAM, or MMIO accesses the way the DMG-family external-source policy does.
- GDMA is modeled as a full-burst transfer and HDMA as a windowed block transfer inside the shared DMA controller, with CPU-impact and advance-condition policy published to the scheduler and bus rather than re-encoded in CGB-only bus branches.
- The timer implementation is expressed in terms of an internal counter plus derived edge logic so CGB clocking changes extend the same model rather than replace it.
- The APU derives `DIV-APU` / frame-sequencer timing from the shared divider timeline so CGB double-speed audio timing extends the same ownership split rather than introducing a second audio clock model.
- The DMG-family OAM corruption bug should stay behind an explicit model gate so CGB, AGB, AGS, and GBP support can keep the documented non-bugged behavior.
- In DMG-family modes, CGB-only MMIO reads should return the correct non-CGB fallback value of `0xFF` instead of emulator-invented placeholders.
- In DMG-family modes, CGB-only MMIO writes should be handled explicitly rather than falling through to fake storage.
- Slice 3 routes `KEY0` / `FF4C`, `VBK` / `FF4F`, `SVBK` / `FF70`, and `FF72-FF75` as implemented CGB-only MMIO through the shared bus contract rather than as generic `FFxx` storage; PPU palettes, `HDMA*`, `OPRI`, `RP`, and `PCM12`/`PCM34` are owned by their later slices rather than by generic CGB storage.
- Slice 3 keeps `FF72`, `FF73`, `FF74`, and `FF75` as distinct per-address register identities. `FF72-FF74` are native-CGB read/write bytes initialized to `$00`, while `FF75` exposes only bits `4-6` as writable and reads back those bits over forced `$8F`.
- The public model surface may already expose an explicit `OperatingMode`, but routed MMIO and subsystem behavior may still stage runtime consultation incrementally; until a specific register path consumes that mode directly, descriptors such as `BGP` / `OBP*` may continue to publish nominal `DMG-compatible` availability without claiming full runtime mode routing.
- CGB boot flow branches into full CGB mode or DMG-compatibility mode based on cartridge header information without requiring a separate emulator core.
- Phase 10 direct boot applies the loaded cartridge header byte at `0x0143` to `ConsoleModel::GameBoyColor` after cartridge load in `SkipBoot`: flags with bit `7` clear select `OperatingMode::GbCompatible`, canonical CGB-supported/CGB-only values select `OperatingMode::Cgb`, strict/permissive noncanonical high-bit values preserve the existing native-CGB fallback without enabling PGB/PSM behavior, experimental policy maps noncanonical bit `3` (`0x88..0x8F`) to `OperatingMode::CgbDmgExt` with bit `3` taking priority over bit `2`, and DMG-family models stay `OperatingMode::Dmg`.
- Phase 10 CGB `SkipBoot` seeds CPU entry registers from the standard CGB boot handoff contract: DMG-only compatibility headers use `AF=$1180`, `BC=$0000`, `DE=$0008`, `HL=$007C`, `SP=$FFFE`, `PC=$0100`, while CGB-supported/native headers use `AF=$1180`, `BC=$0000`, `DE=$FF56`, `HL=$000D`, `SP=$FFFE`, `PC=$0100`.
- CGB `RealBoot` operating-mode handoff is boot-owned: the loader must not fake native/compatible mode before firmware executes, the CGB boot ROM writes `KEY0`, and the explicit `FF50` newly-unmapped edge locks that `KEY0` state and updates machine, bus, speed, and PPU operating-mode state together. Under experimental policy only, `KEY0` bit `3` selects `OperatingMode::CgbDmgExt`; bit `2` remains ordinary `GbCompatible`; after lock, runtime `KEY0` reads remain `$FF` and writes are ignored.
- Phase 10 Slice 2 introduces `SpeedController` as the explicit owner of `KEY1` / `FF4D` and CGB speed state. The controller is active only on CGB-family silicon running native `OperatingMode::Cgb` or experimental `OperatingMode::CgbDmgExt`; Non-CGB models and CGB-family `GbCompatible` mode read `KEY1` as `0xFF`, ignore writes, and publish normal speed to shared consumers.
- The CGB direct-boot timer contract is header-aware for validated buckets: missing or DMG-compatible headers seed visible `DIV` to `0x26` and the hidden timer counter to `0x2674`, matching Mooneye `misc/boot_div-cgbABCDE.gb`; native CGB non-Nintendo old-licensee headers seed visible `DIV` to `0x1E` and hidden counter `0x1E84`, matching gb-cycle's standard `cgb_boot.bin` handoff observation for Ashiepaws `bully.gb`; native CGB old-licensee `$33` headers with binary-zero new-licensee bytes seed visible `DIV` to `0x1E` and hidden counter `0x1E98`, matching Nitro2k01 `whichboot.gb`. Slice 6 keeps RealBoot on canonical `cgb_boot.bin` by seeding CGB RealBoot power-on counter phase to `0xFFFB` and applying only header-bucket-specific `FF50` handoff corrections, so executed firmware reaches the same centralized CGB direct-start handoff timer buckets without runner overlays.
- Native CGB `KEY1` reads keep unused bits high (`0x7E`), publish current speed in bit `7`, and publish the armed prepare bit in bit `0`. Writes latch only bit `0`; all other written bits are ignored.
- A `STOP` executed while `KEY1` is armed follows the CGB speed-switch path: fetch the padding byte, reset the shared divider through the speed-aware divider-reset edge path without the CPU-`DIV`-write-only APU frame-sequencer offset, clear the prepare bit, toggle normal/double speed, and enter the explicit `65_540` scheduler T-cycle speed-switch pause before resuming from the post-padding `PC`; this local timing constant is anchored by Daid `speed_switch_timing_ly.gbc` and `speed_switch_timing_stat.gbc` and may be revisited only with stronger hardware evidence.
- During the speed-switch pause, the scheduler treats CPU-visible domains as stop-active: CPU bus traffic is absent and timer/serial/APU advancement is gated, while the LCD/PPU scan domain continues at normal dot cadence with the CGB STOP visible-output contract active for the pause.
- The shared speed-domain contract is centralized in `CgbSpeedMode`: normal speed and double speed both advance the timer counter by `1` per CPU-visible scheduler T-cycle so CPU-visible `DIV` reads match Daid `speed_switch_timing_div.gbc`, the APU frame sequencer derives its undoubled domain from counter bit `12` in normal speed and bit `13` in double speed, and the baseline internal serial edge consumes bit `8` in normal speed and bit `7` in double speed.
- Phase 10 Slice 7 extends serial's CGB-family `SC.1` latch on top of that same speed-domain contract for native `OperatingMode::Cgb` and experimental `OperatingMode::CgbDmgExt`: low-speed internal serial uses bit `8` in normal speed and bit `7` in double speed, while high-speed internal serial uses bit `3` in normal speed and bit `2` in double speed; DMG-family models and CGB-family `GbCompatible` mode keep `SC.1` non-functional and reading high.
- LCD/PPU timing must continue to use its own scheduler-domain contract; after a completed switch into double speed, the LCD domain ticks every other CPU-visible scheduler T-cycle, and double speed must not be modeled as a generic frame, LY, STAT, or LCD-dot multiplier.
- CGB-family `STOP` entered outside PPU Mode `3` forced blank fills the visible output with CGB RGB555 black through panel shade `3`, while DMG-family `STOP` keeps shade `0` (white); CGB-family `STOP` entered during Mode `3` preserves the currently displayed RGB555 framebuffer because the CGB PPU keeps displaying the same data and can still access VRAM in that phase.
- The Phase 10 `cgb-speed` ROM suite is manifest-backed; Daid `stop_instr.gb (GBC)` is a blocking absolute grayscale comparison decoded from the CGB RGB555 framebuffer, `stop_instr_gbc_mode3.gb` is a blocking rank-normalized RGB555 framebuffer fixture against the SameBoy/GBEmulatorShootout PASS screen, and `speed_switch_timing_div.gbc`, `speed_switch_timing_ly.gbc`, and `speed_switch_timing_stat.gbc` are blocking rank-normalized RGB555 framebuffer fixtures.
- Phase 10 Slice 3 implements native-CGB VRAM and WRAM banking in the bus/memory layer: `VBK` selects CPU-visible VRAM bank `0` or `1` with `$FE | bank` readback, `SVBK` stores the written low three bits with `$F8 | value` readback, and an effective `SVBK` value of `0` maps bank `1` into `D000-DFFF` and its echo alias.
- CGB bank switching is enabled when `ConsoleModel::GameBoyColor` is running native `OperatingMode::Cgb` or experimental `OperatingMode::CgbDmgExt`; DMG-family models read these CGB-only MMIO registers as `$FF`, while CGB-family `GbCompatible` mode keeps bank writes disabled but still exposes the boot-HWIO-visible CGB silicon readbacks that survive compatibility handoff (`VBK=$FE`, `FF72=$00`, `FF73=$00`, `FF75=$8F`, with `SVBK` and `FF74` reading `$FF`).
- Slice 3 established the lockable `KEY0` state shape and Slice 6 closes the real-boot path: `SkipBoot` synthesizes locked `KEY0` state from the cartridge header, `RealBoot` accepts boot ROM writes before `FF50`, the `FF50` handoff locks the boot-written value, ordinary runtime `KEY0` reads remain `$FF`, and post-lock writes are ignored.
- Experimental CGB DMG-ext mode is deliberately narrower than PGB/PSM: when enabled by `CompatibilityPolicy::experimental()` / `HeuristicPolicy::AllowExperimental`, it exposes `BCPS`, `OCPS`, `OPRI` latch/readback, `VBK`, `SVBK`, `KEY1`, `RP`, serial `SC.1`, `PCM12`/`PCM34`, `FF72`, `FF73`, `FF74`, and `FF75`, while blocking `BCPD`, `OCPD`, `HDMA1..HDMA5`, native CGB tile attributes, native CGB palette data mutation, PSM NMI, boot-ROM remap side effects, external-LCD/PGB presentation behavior, and unverified live `OPRI` visual priority switching. Its PPU rendering path remains a DMG-software path with CGB compatibility RGB555 palette adaptation, not a native-CGB tile-attribute path.
- Experimental CGB DMG-ext direct boot also follows the DocBoy `dmg_ext_mode` startup observation that the powered CGB APU register image is visible while boot-logo pulse residue is inactive; gb-cycle therefore clears only the CGB DMG-ext startup channel-active mask and leaves ordinary promoted CGB startup audio unchanged.
- SameBoy corroborates the existence of a PGB-adjacent implementation path by exposing `KEY0`, `OPRI`, `PSM`, `PSWX`, `PSWY`, `PSW`, and related PGB register hooks, and by using a KEY0 bit-3-like state to permit non-ordinary `OPRI` behavior.
- ares corroborates the same direction at implementation level by treating `KEY0` bit `3` as an `opriEnable` / PGB-like gate. These emulator hooks are reference evidence only; they do not replace hardware research or justify PSM NMI, boot-ROM remap side effects, external-LCD/PGB visuals, or live post-boot `OPRI` visual switching in gb-cycle.
- Phase 10 Slice 4 begins with PPU-owned CGB palette MMIO: native CGB routes `BCPS`/`BCPD` and `OCPS`/`OCPD` through separate background/object `64`-byte RGB555 palette RAM, index reads force bit `6` high, data writes auto-increment only when bit `7` is set, and Mode `3` blocks data reads/writes while preserving the documented failed-write auto-increment.
- Native CGB palette data MMIO and palette data writes are enabled only when `ConsoleModel::GameBoyColor` is running `OperatingMode::Cgb`; experimental `OperatingMode::CgbDmgExt` exposes only the `BCPS`/`OCPS` index latches and readbacks while keeping `BCPD`/`OCPD` unavailable, DMG-family models keep all palette ports unavailable, and CGB-family `GbCompatible` mode exposes the boot-HWIO-visible index readbacks (`BCPS=$C8`, `OCPS=$D0` after the centralized compatibility palette seed) but keeps palette data reads at `$FF` and ordinary data/index writes from mutating the compatibility palette seed.
- Phase 10 Slice 4B latches CGB BG/window tile-map attributes from VRAM bank `1` alongside the corresponding tile-number entry from VRAM bank `0`, preserves the raw attribute byte including writable/readable ignored bit `4`, uses bit `3` to select the tile-data VRAM bank, applies horizontal and vertical flips to the logical two-bit color index path, and carries palette index plus BG priority as fetcher/FIFO sideband for RGB555 and BG/OBJ composition work.
- Phase 10 Slice 4C consumes CGB OBJ attributes from the Mode `3` live OAM metadata path, uses bit `3` to select OBJ tile data from VRAM bank `0` or `1`, applies horizontal and vertical flip bits before producing logical two-bit OBJ color indices, preserves OBJ palette index and BG-over-OBJ priority as OBJ FIFO sideband, and keeps color index `0` transparent before palette lookup and final BG/OBJ composition.
- Phase 10 Slice 4D adds a CGB framebuffer surface containing raw logical RGB555 pixels: visible CGB BG/window pixels look up `BCPD` palette RAM through the latched BG attribute palette index, visible CGB OBJ pixels look up `OCPD` palette RAM through the latched OBJ attribute palette index, OBJ color index `0` still resolves as transparency before any OBJ palette lookup, and the legacy shade framebuffer remains a DMG/debugging surface rather than the CGB presentation or oracle path. Compatibility-mode RGB555 routing is handled by the Slice 4F adapter.
- Phase 10 Slice 4E implements the native CGB BG/OBJ priority composer: OBJ/OBJ drawing priority is selected by a boot-latched mode (`CGB` native uses OAM order, CGB compatibility and experimental CGB DMG-ext use DMG-style X coordinate without enabling DMG silicon quirks), `OPRI` / `FF6C` is an implemented native-CGB and CGB-DMG-ext MMIO latch with `$FE | bit0` readback but ordinary post-boot writes do not mutate visual priority, and final native-CGB BG-over-OBJ composition follows CGB `LCDC.0`, BG attribute bit `7`, OAM attribute bit `7`, and BG color-index `0` rules.
- Phase 10 Slice 4F implements the direct-boot CGB compatibility palette adapter: `SkipBoot` resolves the standard CGB boot lookup from Nintendo licensee detection, the 16-byte title checksum, ambiguous fourth-title-byte correction, lookup-index `0` fallback, and the held-joypad boot-input override table, then seeds BG palette `0` and OBJ palettes `0`/`1` while runtime compatibility RGB555 maps the visible DMG-software `BGP`, `OBP0`, and `OBP1` view through those fixed CGB palette slots, with explicit Mode `3` `BGP`/`OBP*` conflict repaint paths supplying override palettes for already-presented pixels. Ordinary RGB555 BG output uses the visible `BGP` latch unless a BGP visible hold is explicitly active; it does not inherit the DMG-family `visible | pipeline` BGP panel fallback, and the DMG panel output-override delay is limited to selected-OBJ scanlines where the CGB Mealybug evidence exposes a sprite-coupled seam. The adapter is also used by experimental `CgbDmgExt`; CGB-family DMG-software `BGP` Mode `3` commits use CGB-specific transient/onset rules where Mealybug evidence distinguishes them from DMG-family panel behavior, CGB-family DMG-software `OBP*` conflicts reuse the DMG current-line OBJ recolor span through the adapter, native CGB keeps skipping DMG palette-conflict paths, and the DMG-family previous-scanline boundary repaint seam remains DMG-only.
- CGB-family DMG-software `SCX` Mode `3` writes share the DMG software contract in both `GbCompatible` and experimental `CgbDmgExt`, including pre-visible low-bit discard retuning; Mealybug CGB evidence also distinguishes startup `VisibleTile3` high-bit writes from native CGB by preserving the old carried tile for the affected early seam windows.
- CGB-family DMG-software `SCY` Mode `3` writes also share the DMG software contract at the MMIO/API level, but CGB silicon exposes a different BG fetcher seam from DMG for `m3_scy_change`: the high bitplane reuses the low bitplane's tiledata row when a write lands on the low/high plane tiledata seam, and the startup left-sprite row-retarget table is CGB-family specific for `GbCompatible` and `CgbDmgExt` instead of reusing the DMG startup-alignment FIFO latch wholesale.
- CGB-family DMG-software `WX` Mode `3` writes share the DMG software register contract for `GbCompatible` and experimental `CgbDmgExt`, but the low-`WX` same-line reactivation seam is CGB-family specific: non-cancel DMG previsible retargets are suppressed, cancel-only low-`WX` aborts preserve the current line start count, only tile-index-phase previsible reactivation can insert a raw color-`0` FIFO pixel, later visible `WX` writes clear that pending previsible insertion instead of arming another CGB raw-zero seam, and the `WX=4`/`WX=5`/`WX=6` phase repaint is bounded and cancelable by a later `WX` restore before its phase guard. The `WX=4 -> WX=5` fixed-prefix repaint samples the current prefix once before repainting and degenerates to a no-op only when that pre-repaint prefix is already all color `3`. The `WX=4` repaint includes CGB-family plane-source seams where phase `0` preserves the current high plane while borrowing the delayed window high plane as the low plane, phase `2` copies the current low plane into the high plane, and phase `6` copies delayed window pixels.
- CGB-family DMG-software `LCDC.5` Mode `3` writes remain distinct from `WX`-only same-line retriggers: a second `WX` trigger is suppressed while the fetcher is still windowed, but a later `LCDC.5` re-enable can start the window again once the earlier disable has aborted the fetcher back to background and a new not-yet-served `WX` trigger is reached. The experimental CGB-C/D `m3_lcdc_win_en_change_multiple_wx` oracle further indicates bounded repaint artifacts, not fetcher restarts and not window-line-counter increments: low-`WX` disables at `WX=0..2` keep longer left prefixes, early re-enables for `WX=2,4,5,6,7,8,9` and later second-enable rows for `WX=32..36` repaint fixed panel-shade patterns, initial missed starts repaint around `WX=18..22`, selected resume tails repaint around `WX=21,22,28,29,30,32,35,36`, and late full-tail re-enables repaint around `WX=46..48`.
- CGB-family DMG-software `LCDC.0` and `LCDC.3` Mode `3` writes likewise share the DMG software contract at the register/API level while using CGB-family timing seams: `LCDC.0` BG-disable forced-white dots must remain panel/RGB555 white while restored BG dots use the CGB compatibility adapter, and `LCDC.3` BG-map startup retargeting uses a CGB-specific low-sprite phase table for `GbCompatible` and `CgbDmgExt`.
- CGB-family DMG-software `LCDC.1` Mode `3` writes also share the DMG software contract at the register/API level while using CGB-family timing seams: OBJ-disable visible holds and retroactive repaints use a distinct single-left-sprite onset table for `GbCompatible` and `CgbDmgExt`, and the `m3_lcdc_obj_en_change_variant` BGP timing pulse uses CGB-specific late onsets plus visible-BG-pixel holds through the compatibility RGB555 adapter. While a BGP visible hold is pending, both ordinary BG dots and any overlapping `LCDC.1` repaint use the held BGP value for the legacy panel shade and the RGB555 compatibility adapter rather than the already-committed fallback palette, keeping the two framebuffer surfaces coherent.
- Phase 10 Slice 4G locks the current CGB BG/window fetcher latch contract with focused probes: tile number and VRAM-bank-1 attributes are sampled on the tile-index read dot, the latched palette/flip/priority/tile-bank sideband stays stable for already-fetched pixels, later attribute writes are observed only by later fetches, CPU `VBK` selection does not retarget PPU banked fetches, window restarts latch the window attribute-map entry instead of reusing stale BG attributes, and both window tile-data planes apply the same latched CGB attributes for VRAM-bank and vertical-flip selection.
- Phase 10 Slice 7 routes `RP` / `FF56` as a CGB-family infrared register baseline for native `OperatingMode::Cgb` and experimental `OperatingMode::CgbDmgExt`, with bit `0` as the emitter latch, bits `6-7` as read-enable latches, bits `2-5` reading high, bit `1` reading no-signal high unless read enable is `$C0` and the deterministic IR sensor currently sees an effective signal, and DMG-family plus CGB compatibility mode fallback to `$FF`.
- Post-Slice 10 CGB IR models native CGB optical input as bus-owned `RP` sensor state: the sensor sees external light supplied by either the two-machine CGB IR topology or a single-machine accessory session, ORs that input with its own emitter for local self-visibility, advances once per scheduler T-cycle with provisional warmup/fade constants cross-checked against SameBoy and GBE+, and treats an already read-enabled sensor as immediately ready for short peer/accessory pulses because Shonumi's hardware notes say the re-enable delay does not apply when bits `6-7` stay asserted. The two-CGB linked topology, `PokemonPikachuColorSession`, and `PokemonMysteryGiftSession` sample `RP` emitters during `ExternalEventIngress` and route optical light through explicit topology/accessory delay values rather than making a CPU write to `RP` visible to external hardware immediately or in the same CPU cycle.
- `Machine::cgb_infrared_status` exposes a non-mutating CGB-family observability surface for frontends and debuggers when `RP` is live in native CGB or experimental CGB DMG-ext: it reports the `RP` latch, emitter state, receiver enable, peer optical input, effective sensor light, warmup counter/readiness, and bit-`1` visibility without performing an MMIO read or changing sensor timing.
- Phase 10 Slice 7 routes `PCM12` / `FF76` and `PCM34` / `FF77` as CGB-family read-only APU digital-output taps, with low/high nibbles mapped to CH1/CH2 and CH3/CH4 respectively; DMG-family models keep those CGB-only registers unavailable, while CGB-family `GbCompatible` and experimental `CgbDmgExt` modes expose the same read-only taps and therefore start from `$00` when all four digital outputs are idle.
- CGB-family `LY` / `LYC` timing includes the late line-`153` `LY=0` window used by compatibility-mode raster effects: from dot `8` of line `153`, `FF44` readback and `LYC` comparison expose `0` even though the internal raster snapshot still reports line `153`, allowing `LYC=0` STAT handlers to begin before visible line `0`.
- When CGB work begins, prefer a single standard CGB model entry point before considering hardware revision variants.
- A CGB running a DMG title should be treated as the shared core operating with CGB-only features disabled by mode, not as a separate emulator path; the experimental `CgbDmgExt` exception enables only the documented narrow register subset while preserving the DMG software and PPU contracts.

## Deferred for now

These can stay unimplemented in the first DMG-family core as long as the architecture leaves them a clear place:

- post-boot `OPRI` visual priority mutation beyond the implemented latch/readback baseline, unless backed by dedicated hardware evidence
- full PGB/PSM behavior, PSM NMI, boot-ROM remap side effects after normal handoff, external-LCD/PGB visual behavior, and undocumented live `KEY0`/`OPRI` interactions beyond the experimental CGB DMG-ext register profile
- CGB0/CGB-E revision-specific boot-ROM behavior beyond strict asset recognition for future validation
- linked CGB serial transport beyond single-console `SC.1` high-speed transfer timing
- host-side infrared light injection, non-CGB IR devices other than the modeled `PokemonPikachuColor` and `PokemonMysteryGift` accessories, HuC1/HuC3-to-CGB IR, title-specific external IR protocols, and hardware-revision-specific analog sensor tuning beyond the provisional CGB-to-CGB timing constants
- CGB OAM DMA duration differences in double speed are now owned by `hardware/DMA.md`: `FF46` latches the current CGB speed profile, keeps the `160` CPU M-cycle DMA body, and exposes the double-speed LCD-domain dot difference through the shared scheduler speed contract.
- HDMA and GDMA core execution are now owned by `hardware/DMA.md`; Slice 5 promotes the initial SameSuite `cgb-dma` rows to blocking framebuffer fixtures and locks the active-HDMA live-bus bank/`VBK` plus HBlank seam policies there.
- AGB/AGS/GBP and other post-CGB boot-ROM variants
- DMG-on-CGB compatibility details
- functional support for any remaining CGB-only special cartridges beyond the now-supported `MBC30`, `MBC6`, and focused MBC7 sensor / EEPROM path

## Known pitfalls

- coupling DMG assumptions into shared APIs
- hiding double-speed effects behind generic timing helpers
- over-designing around CGB revision differences before the base CGB feature set exists
- assuming DMG-family OAM corruption should also exist on CGB-family hardware running DMG software

## Open questions

- which shared abstractions can remain stable across DMG and CGB without losing clarity
