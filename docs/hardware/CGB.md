# CGB

## Scope

Own Color Game Boy-specific behavior: double speed, VRAM banks, WRAM banks, palettes, HDMA, and other model-specific extensions beyond shared DMG behavior.

## Hardware model

Design interfaces today that do not block CGB tomorrow. Separate DMG-only, shared, and CGB-only behavior explicitly.

CGB should extend the shared core through model-aware behavior and capabilities, not by introducing a parallel emulator architecture.
Until CGB work starts, avoid premature complexity in DMG-family subsystems; only preserve the extension seams that prevent large future refactors.

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
- `ConsoleModel::Cgb` plus `OperatingMode::CgbCompatibility` should mean "CGB-family silicon running monochrome software-visible mode", not "pretend this machine is a DMG".
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
- When CGB work begins, prefer a single standard CGB model entry point before considering hardware revision variants.
- A CGB running a DMG title should be treated as the shared core operating with CGB-only features disabled by mode, not as a separate emulator path.

## Deferred for now

These can stay unimplemented in the first DMG-family core as long as the architecture leaves them a clear place:

- real CGB palettes
- VRAM bank 1 behavior
- WRAM banks 2-7
- `KEY0` boot-time semantics and lock behavior
- `KEY1` and double speed behavior
- APU `DIV-APU` / frame-sequencer behavior under CGB double speed
- timer behavior under CGB double-speed timing
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
