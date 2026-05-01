# CGB

## Scope

Own Color Game Boy-specific behavior: double speed, VRAM banks, WRAM banks, palettes, HDMA, and other model-specific extensions beyond shared DMG behavior.

## Hardware model

Design interfaces today that do not block CGB tomorrow. Separate DMG-only, shared, and CGB-only behavior explicitly.

CGB should extend the shared core through model-aware behavior and capabilities, not by introducing a parallel emulator architecture. Until CGB work starts, avoid premature complexity in DMG-family subsystems; only preserve the extension seams that prevent large future refactors.

`ConsoleModel` is the visible product model, not the CPU revision string and not the boot ROM filename. The CGB product row is `ConsoleModel::GameBoyColor`; it defaults to `BootRomKind::Cgb`, allows `Cgb0`, `Cgb`, and `CgbE` for `RealBoot`, and uses the CGB skip-boot synthetic startup profile when `StartupMode::SkipBoot` is selected. CPU revisions such as `CPU CGB 0`, `CPU CGB A`, `CPU CGB C`, `CPU CGB D`, and `CPU CGB E` are documented hardware profiles only in this phase and must not enter behavior gates until a tested revision-specific difference is intentionally modeled.

The cross-family model-profile table that aligns `ConsoleModel`, `HostPlatform`, CPU revision names, boot ROM filenames, operation mode, and display/color profile lives in [`core/MODEL-AXES.md`](../core/MODEL-AXES.md#reference-model-profiles). This CGB handbook owns the CGB behavior that consumes those axes, not the global product taxonomy.

The desktop presentation palettes for `GameBoy`, `GameBoyPocket`, and `GameBoyLight` intentionally live in `gb-desktop`, not `gb-core`: the core framebuffer remains shade/rank data for tests and tooling, while desktop rendering and screenshots map shades through SameBoy's `Core/display.c` DMG, MGB, and GBL palettes. Shade mapping follows SameBoy: shade `0` uses palette entry `3`, shade `1` uses entry `2`, shade `2` uses entry `1`, and shade `3` uses entry `0`. `GameBoyColor` is reserved for native color rendering once CGB palettes are implemented and currently keeps the existing monochrome fallback path from breaking.

## Responsibilities

- double-speed behavior
- banked VRAM and WRAM behavior
- color palettes
- CGB-only DMA/HDMA features
- model capability flags and feature gates
- CGB boot-time mode selection versus DMG-compatibility mode

## Implementation priority

When CGB work starts, prioritize these functional areas before worrying about hardware revision variants:

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
- undocumented `FF72`, `FF73`, `FF74`, `FF75`

## DMG fallback policy for CGB-only MMIO

- The shared MMIO map should record, per register, both nominal availability and current implementation state.
- In the current DMG-only but CGB-ready baseline, nominal availability should at least distinguish shared, DMG-compatible, and CGB-only registers.
- In that same baseline, current implementation state should distinguish implemented, stubbed, and unavailable registers without forcing the bus to fake full CGB support.
- In DMG mode, CGB-only registers that are not functionally implemented should return the documented non-CGB fallback value, typically `0xFF`, rather than behaving as RAM.
- Writes to those registers in DMG mode should follow an explicit ignored-or-DMG-semantics policy and must not mutate nonexistent state accidentally.
- Bringing CGB support online later should extend the same routed MMIO contract rather than replacing a temporary DMG-only shortcut.

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
- when CGB work starts, negative tests that DMG-family OAM corruption behavior does not appear on CGB-family hardware even while running monochrome software

## Implementation notes for this repo

- Model capabilities should be centralized, not spread as random conditionals.
- The public model surface should distinguish at least `ConsoleModel`, `OperatingMode`, and `HostPlatform` so CGB silicon, CGB native mode, CGB DMG-compatibility mode, and future SGB hosting are not conflated.
- Shared subsystems should expose clean extension points for CGB-only behavior.
- DMG-family behavior should remain the baseline shared path where possible, with CGB-specific features layered on through explicit model capabilities.
- `ConsoleModel::GameBoyColor` plus `OperatingMode::GbCompatible` should mean "CGB-family silicon running monochrome software-visible mode", not "pretend this machine is a DMG".
- CGB readiness today should focus on architecture seams for banked memory, palette state, extra I/O, HDMA, and speed switching, not on partial functional implementation.
- Do not claim functional closure for CGB-only special cartridges such as `MBC30`, `MBC7`, or `MBC6` before the base CGB implementation can boot and validate CGB-only software end to end; before that point, keep only explicit classification, typed variant space, and clear TODO tracking.
- The shared CPU execution model should already be based on in-flight fetch/read/write/internal steps so future double-speed behavior can scale the same engine instead of replacing an opcode-duration-based core.
- CPU `STOP` should already be represented separately from `HALT`, because future CGB speed-switch behavior should attach to an existing explicit control state rather than force a later CPU-state redesign.
- The boot subsystem and bus should already treat boot-ROM mapping as model-aware routing state so future CGB split boot-ROM windows can extend the same abstraction while preserving cartridge-header visibility around `0x0100-0x014F`.
- The DMG OAM DMA implementation should already live inside a reusable DMA subsystem contract so future CGB OAM DMA timing differences, GDMA, and HDMA can extend the same infrastructure.
- When CGB DMA work starts, model GDMA as a full-burst transfer and HDMA as a windowed block transfer inside that shared DMA controller, with CPU-impact and advance-condition policy published to the scheduler and bus rather than re-encoded in CGB-only bus branches.
- The DMG timer implementation should already be expressed in terms of an internal counter plus derived edge logic so future CGB clocking changes can extend the same model rather than replace it.
- The DMG APU implementation should already derive `DIV-APU` / frame-sequencer timing from the shared divider timeline so future CGB double-speed audio timing can extend the same ownership split rather than introducing a second audio clock model.
- The DMG-family OAM corruption bug should stay behind an explicit model gate so future CGB, AGB, AGS, and GBP support can keep the documented non-bugged behavior.
- In DMG mode before functional CGB support exists, CGB-only MMIO reads should already return the correct non-CGB fallback value of `0xFF` instead of emulator-invented placeholders.
- In DMG mode before functional CGB support exists, CGB-only MMIO writes should already be handled explicitly rather than falling through to fake storage.
- Even before functional CGB work starts, routed MMIO metadata should already classify `KEY0` / `FF4C` as an explicit CGB-only register rather than as a reserved shared-model hole.
- Even before functional CGB work starts, the routed MMIO metadata should keep nominal future ownership for stubbed CGB registers, such as PPU-owned palettes, DMA-owned `HDMA*`, and system-owned `KEY0` / `KEY1`, rather than collapsing all of them into one generic "CGB-only owner".
- Even before functional CGB work starts, the routed MMIO metadata should keep `FF72`, `FF73`, `FF74`, and `FF75` as distinct per-address register identities. In the current descriptor baseline, `FF72-FF74` should publish nominal `ReadWrite` access, while `FF75` should remain `Mixed` because only bits `4-6` are writable in CGB mode.
- The public model surface may already expose an explicit `OperatingMode`, but routed MMIO and subsystem behavior may still stage runtime consultation incrementally; until a specific register path consumes that mode directly, descriptors such as `BGP` / `OBP*` may continue to publish nominal `DMG-compatible` availability without claiming full runtime mode routing.
- Future CGB boot flow should be able to branch into full CGB mode or DMG-compatibility mode based on cartridge header information, without requiring a separate emulator core.
- Phase 10 direct boot applies the loaded cartridge header byte at `0x0143` to `ConsoleModel::GameBoyColor` after cartridge load in `SkipBoot`: flags with bit `7` clear select `OperatingMode::GbCompatible`, canonical CGB-supported/CGB-only values select `OperatingMode::Cgb`, noncanonical high-bit values select native CGB without enabling PGB/PSM behavior, and DMG-family models stay `OperatingMode::Dmg`.
- Phase 10 CGB `SkipBoot` seeds the CPU entry registers to the `boot_regs-cgb` contract (`AF=$1180`, `BC=$0000`, `DE=$0008`, `HL=$007C`, `SP=$FFFE`, `PC=$0100`) so direct boot starts from the same observable CGB register baseline before real boot-ROM handoff is implemented.
- CGB `RealBoot` operating-mode handoff remains boot-owned until the Slice 6 boot-ROM work validates `KEY0`, `FF50`, and RealBoot versus SkipBoot equivalence; do not fake a real-boot mode switch from the loader path before that handoff exists.
- Phase 10 Slice 2 introduces `SpeedController` as the explicit owner of `KEY1` / `FF4D` and CGB speed state. The controller is active only on CGB-family silicon running native `OperatingMode::Cgb`; Non-CGB models and CGB-family `GbCompatible` mode read `KEY1` as `0xFF`, ignore writes, and publish normal speed to shared consumers.
- Native CGB `KEY1` reads keep unused bits high (`0x7E`), publish current speed in bit `7`, and publish the armed prepare bit in bit `0`. Writes latch only bit `0`; all other written bits are ignored.
- A `STOP` executed while `KEY1` is armed follows the CGB speed-switch path: fetch the padding byte, reset the shared divider through the speed-aware `DIV` write-effect path, clear the prepare bit, toggle normal/double speed, and enter the explicit `65_540` scheduler T-cycle speed-switch pause before resuming from the post-padding `PC`; this local timing constant is anchored by Daid `speed_switch_timing_ly.gbc` and `speed_switch_timing_stat.gbc` and may be revisited only with stronger hardware evidence.
- During the speed-switch pause, the scheduler treats CPU-visible domains as stop-active: CPU bus traffic is absent and timer/serial/APU advancement is gated, while the LCD/PPU scan domain continues at normal dot cadence with the CGB STOP visible-output contract active for the pause.
- The shared speed-domain contract is centralized in `CgbSpeedMode`: normal speed and double speed both advance the timer counter by `1` per CPU-visible scheduler T-cycle so CPU-visible `DIV` reads match Daid `speed_switch_timing_div.gbc`, the APU frame sequencer derives its undoubled domain from counter bit `12` in normal speed and bit `13` in double speed, and the baseline internal serial edge consumes bit `8` in normal speed and bit `7` in double speed.
- LCD/PPU timing must continue to use its own scheduler-domain contract; after a completed switch into double speed, the LCD domain ticks every other CPU-visible scheduler T-cycle, and double speed must not be modeled as a generic frame, LY, STAT, or LCD-dot multiplier.
- CGB-family `STOP` entered outside PPU Mode `3` forced blank fills the visible framebuffer with panel shade `3` (black), while DMG-family `STOP` keeps shade `0` (white); CGB-family `STOP` entered during Mode `3` preserves the currently displayed framebuffer because the CGB PPU keeps displaying the same data and can still access VRAM in that phase. This is a small STOP/visible-output bridge for current monochrome framebuffer data, not the full Slice 4 CGB palette renderer.
- The Phase 10 `cgb-speed` ROM suite is manifest-backed; Daid `stop_instr.gb (GBC)` is a blocking absolute grayscale framebuffer fixture, `stop_instr_gbc_mode3.gb` is a blocking rank-normalized framebuffer fixture against the SameBoy/GBEmulatorShootout PASS screen, and `speed_switch_timing_div.gbc`, `speed_switch_timing_ly.gbc`, and `speed_switch_timing_stat.gbc` are blocking rank-normalized framebuffer fixtures.
- When CGB work begins, prefer a single standard CGB model entry point before considering hardware revision variants.
- A CGB running a DMG title should be treated as the shared core operating with CGB-only features disabled by mode, not as a separate emulator path.

## Deferred for now

These can stay unimplemented in the first DMG-family core as long as the architecture leaves them a clear place:

- real CGB palettes
- VRAM bank 1 behavior
- WRAM banks 2-7
- `KEY0` boot-time semantics and lock behavior
- strict `cgb-speed` oracle promotion for the remaining Daid framebuffer cases and Blargg timing output
- full CGB serial `SC.1` high-speed transfer behavior beyond the Slice 2 shared speed-domain edge contract
- CGB OAM DMA duration differences in double speed
- HDMA and GDMA
- CGB tile attributes
- CGB boot ROM behavior
- DMG-on-CGB compatibility details
- functional support for CGB-only special cartridges such as `MBC30`, `MBC7`, and `MBC6`

## Known pitfalls

- coupling DMG assumptions into shared APIs
- hiding double-speed effects behind generic timing helpers
- over-designing around CGB revision differences before the base CGB feature set exists
- assuming DMG-family OAM corruption should also exist on CGB-family hardware running DMG software

## Open questions

- which shared abstractions can remain stable across DMG and CGB without losing clarity
