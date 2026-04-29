# Model Axes

## Scope

Explain how to use the public model-facing types introduced around `ConsoleModel`, `OperatingMode`, `HostPlatform`, and `CapabilitySet`.

This file is a code-facing usage and migration note. It exists to keep DMG, future CGB, and future SGB work from collapsing distinct concepts back into one catch-all enum.

## Authority boundaries

- `ARCHITECTURE.md` owns the existence of the separate axes and the high-level architectural reason for them.
- `hardware/CGB.md`, `hardware/SGB.md`, and `hardware/BOOT-ROM.md` own the subsystem behavior that later consumes those axes.
- This file owns the practical "which type should I consult here?" guidance for production code and follow-up refactors.

If this file conflicts with a subsystem handbook about hardware truth, the subsystem handbook wins.

## Mental model

Treat the public model surface as independent axes, with `BootRomKind` deliberately kept outside the model identity:

```text
ConsoleModel   = visible product model selected by users and frontends
OperatingMode  = which GB-visible mode the software is currently running under
HostPlatform   = which outer host shell surrounds the shared GB core
BootRomKind    = which firmware image RealBoot executes
```

Examples:

- `ConsoleModel::GameBoy` + `BootRomKind::Dmg` + `OperatingMode::Dmg` + `HostPlatform::Handheld` = ordinary Game Boy with the standard DMG boot ROM
- `ConsoleModel::GameBoy` + `BootRomKind::Dmg0` = the same visible Game Boy product model with the earlier DMG0 firmware selected for `RealBoot`
- `ConsoleModel::GameBoyPocket` or `ConsoleModel::GameBoyLight` + `BootRomKind::Mgb` + `OperatingMode::Dmg` = DMG-family handheld product using the MGB boot profile
- `ConsoleModel::GameBoyColor` + `OperatingMode::Cgb` = native CGB
- `ConsoleModel::GameBoyColor` + `OperatingMode::GbCompatible` = CGB-family silicon running monochrome software-visible mode
- `HostPlatform::Sgb1` or `HostPlatform::Sgb2` = future SGB shell around the shared GB core, not a different GB silicon family

`CapabilitySet` is the derived semantic view over those axes. It exists so most subsystem code can ask the question it really means instead of manually recomputing it.

## When to use each type

### Use `ConsoleModel` when the question is about the visible product model

Reach for `ConsoleModel` when the code needs to know the user-facing product class and its default hardware-family contract.

Typical uses:

- default boot ROM kind and allowed firmware set
- skip-boot power-up presets that differ by product model
- silicon-family quirks such as DMG-family-only OAM corruption
- product-specific desktop presentation such as DMG, MGB, or Game Boy Light display palette
- raw family classification when a subsystem is defining a derived capability

Do not model CPU revision suffixes such as `DMG-CPU B` or `CPU CGB C` as functional enum values until a tested hardware behavior actually needs them. Keep those revision notes in documentation and choose the closest product-level default.

Do not use `ConsoleModel` just because it is nearby in the API. If the real question is "is this feature enabled right now?", `ConsoleModel` is usually too low-level.

### Use `BootRomKind` when the question is about firmware

Reach for `BootRomKind` when the code needs to load, verify, route, or execute a concrete boot ROM image. `BootRomKind` is selected explicitly for `RealBoot`; `SkipBoot` does not require an asset and instead uses the synthetic startup profile for the selected `ConsoleModel`.

Current defaults and allowed firmware sets are:

| ConsoleModel | Default BootRomKind | Allowed BootRomKind values |
|---|---|---|
| `GameBoy` | `Dmg` | `Dmg0`, `Dmg` |
| `GameBoyPocket` | `Mgb` | `Mgb` |
| `GameBoyLight` | `Mgb` | `Mgb` |
| `GameBoyColor` | `Cgb` | `Cgb0`, `Cgb`, `CgbE` |

### Use `OperatingMode` when the question is about the active software-visible GB mode

Reach for `OperatingMode` when the machine's silicon is not enough to answer the question because the running mode matters.

Typical uses:

- CGB native mode versus CGB compatibility mode
- mode-dependent MMIO visibility or routing
- mode-dependent palette behavior
- mode-dependent boot handoff on CGB-family hardware
- policy that should differ between "CGB hardware running CGB software" and "CGB hardware running DMG software"

Do not treat `OperatingMode::GbCompatible` as shorthand for DMG silicon. The software contract may look DMG-like while the underlying hardware family is still CGB.

### Use `HostPlatform` when the question is about the outer shell, not the GB silicon

Reach for `HostPlatform` when the behavior belongs to the environment around the shared GB core.

Typical uses:

- future SGB command transport
- future SGB border ownership
- future SGB multiplayer-host behavior
- host-shell timing coordination with a SNES-side implementation

`HostPlatform` should not decide CPU, PPU, DMA, timer, or APU truth directly unless a subsystem handbook later documents a real host-platform-visible effect.

### Use `CapabilitySet` by default for subsystem behavior gates

For most production code, prefer `CapabilitySet` over directly branching on `ConsoleModel`, `OperatingMode`, and `HostPlatform`.

Use it when the question is semantic:

- "does DMG software contract apply?"
- "are CGB extensions enabled?"
- "do DMG-family silicon quirks apply?"
- "are SGB host enhancements active?"

This keeps subsystem code readable and avoids re-encoding the meaning of the axes differently in CPU, PPU, DMA, timer, and boot code.

## Preferred decision order

When adding or refactoring model-aware code:

1. Ask whether the code really wants a semantic capability.
2. If yes, use `CapabilitySet` or add a new derived capability there.
3. If not, ask whether the question is about silicon, active operating mode, or host shell.
4. Use `ConsoleModel`, `OperatingMode`, or `HostPlatform` only for that specific raw concern.

In short:

```text
behavior gate -> CapabilitySet first
silicon fact  -> ConsoleModel
mode fact     -> OperatingMode
host-shell    -> HostPlatform
```

## Concrete examples

Use `CapabilitySet::dmg_family_quirks_enabled()` for:

- DMG-family OAM corruption gating
- other true DMG-family-only silicon quirks

Use `ConsoleModel` directly for:

- deriving the default and allowed `BootRomKind` set
- product-specific skip-boot startup presets
- product-specific desktop display palette selection

Use `BootRomKind` directly for:

- boot ROM asset filenames and hashes
- `RealBoot` firmware selection
- boot ROM mapping payload lookup

Use `OperatingMode` or a capability derived from it for:

- whether CGB palette hardware is actively exposed
- whether CGB-only tile attributes participate in rendering
- whether a CGB running a DMG title should follow DMG-visible rendering rules

Use `HostPlatform` or a capability derived from it for:

- future SGB packet decoder ownership
- future border composition outside the handheld LCD image
- future SGB multiplayer-controller multiplexing

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
- Do not put SGB host-shell policy behind random `ConsoleModel` checks.
- Do not re-derive the same semantic meaning from the raw axes in several subsystems.
- Do not add a second emulator path just because one raw axis is insufficient.

## Review checklist

When a change adds model-aware behavior, verify:

- the branch is using the right axis
- semantic gates prefer `CapabilitySet`
- silicon-only quirks are not accidentally keyed off `OperatingMode`
- host-shell behavior is not leaking into handheld-core logic
- CGB compatibility mode is not being confused with DMG silicon
