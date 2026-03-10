# Architecture

## Goals

- Prioritize hardware-accurate behavior.
- Keep the core modular enough to evolve from DMG-first to CGB and later SGB support.
- Keep portability high so the core remains platform-agnostic.
- Preserve determinism, debuggability, and testability from the start.

## Recommended high-level layout

Preferred long-term structure:

```text
crates/
  gb-core/
    src/
      cpu/
      ppu/
      apu/
      bus/
      dma/
      timer/
      joypad/
      serial/
      cartridge/
      boot/
      model/
      scheduler/
      debugger/
      lib.rs
  gb-test-runner/
  gb-cli/
  gb-desktop/
  gb-web/
tests/
AI/
```

For an early-stage repo, a simplified equivalent is acceptable as long as these boundaries stay visible.

## Rust module layout policy

- Use `foo.rs` for small subsystems.
- When a subsystem grows, prefer `foo.rs` plus `foo/` as the default production layout.
- Treat the top-level subsystem file as the facade for module declarations, re-exports, and narrow orchestration.
- Move hardware responsibilities into focused child files instead of letting timing-sensitive logic accumulate in one large file.
- Avoid layout churn during behavior work; structural migrations should be isolated when possible.

## Core design principles

- Model hardware by responsibilities, not by frontend features.
- Favor explicit state transitions over implicit side effects.
- Keep timing ownership clear.
- Separate behavior specification from optimization strategy.
- Make room for CGB-specific extensions without spreading model checks everywhere.
- Use types to reflect hardware concepts such as model, interrupt source, PPU mode, and cartridge kind.

## Timing foundation

- The project timing foundation is T-cycle based from the start.
- M-cycles may be referenced for documentation or instruction summaries, but they are not the primary execution unit of the core.
- Shared subsystem scheduling should assume a common T-cycle timeline so CPU, PPU, timer, DMA, APU, and bus interactions can be modeled without coarse conversion layers.
- For the PPU, that shared T-cycle timeline is also the dot timeline; dot-by-dot behavior is the intended baseline.
- Long-running hardware operations triggered by MMIO writes, such as OAM DMA, should become explicit in-flight subsystem state on that shared timeline rather than immediate bulk side effects.

## Console model policy

- The core must expose an explicit console model concept.
- At minimum, plan for `DMG0`, `DMG`, `MGB`, and future `CGB`.
- The current implementation target should behave as a DMG-family core, while still distinguishing observable model differences.
- Design the DMG-family core with future CGB integration in mind, but do not introduce CGB implementation complexity before it is needed.
- The goal is to avoid major later refactors, not to prematurely model every CGB-only path.
- Boot ROM behavior and startup-visible quirks must be model-aware rather than treated as one generic DMG state.
- `DMG0`, `DMG`, and `MGB` should share one DMG-family hardware core unless evidence shows a true hardware-level divergence that matters to emulation.
- CGB must enter as an extension of the shared architecture, not as a second emulator with duplicated subsystems.
- No critical subsystem should be rigidly tied to a single hardware variant if that would block natural extension to other models.

## DMG-first, CGB-ready policy

- The base core should implement DMG-family behavior only until DMG timing and correctness are stable.
- "Prepared for CGB" means leaving explicit extension seams, not implementing partial CGB logic ahead of time.
- Shared subsystems should be designed so later support for banked VRAM, banked WRAM, extra CGB I/O registers, HDMA, palette state, and double speed can be added without re-architecting the whole core.
- Avoid rigid fixed-size assumptions in subsystem interfaces when the hardware family naturally extends them later.
- Keep the common GB model solid first; do not dilute DMG timing work by mixing in unfinished CGB behavior.
- When CGB arrives, prefer one standard CGB model before attempting fine-grained CGB hardware revision support.
- Architecture should allow the same core to run in DMG-family mode or CGB mode without duplicating subsystem implementations.

## Suggested subsystem boundaries

- CPU: instruction flow, register state, interrupt acceptance, HALT/STOP semantics
- Bus: address decoding, subsystem routing, dynamic mapping, visible access ordering, and temporal arbitration of blocked accesses
- Memory and MMIO: WRAM, HRAM, echo behavior, plain storage ownership, and MMIO-backed state not owned by another subsystem
- Interrupt controller: IF/IE state and request/acknowledge flow
- Timer: DIV/TIMA/TMA/TAC behavior and edge-sensitive increment logic
- PPU: LCD modes, fetcher/FIFO behavior, rendering state, VRAM/OAM restrictions
- DMA: OAM DMA and future HDMA scheduling and blocking rules
- APU: internal channel/frame-sequencer state only, not output backends
- Joypad and serial: hardware-visible registers and signaling
- Cartridge and MBC: ROM/RAM banking, RTC, rumble, and mapper-specific behavior
- Boot ROM and model config: power-up state, revision differences, direct-boot setup
- Model-specific extensions: CGB and later SGB

## Ownership boundary notes

- The boot subsystem owns firmware assets, model-aware boot configuration, and boot-ROM enable/disable state.
- The DMA subsystem owns transfer state and transfer requests over time.
- The PPU owns LCD mode state and the rules that determine when VRAM/OAM are accessible.
- The bus applies boot mapping, DMA contention, and blocked-access semantics using that subsystem state; CPU code should not embed those rules directly.
- The memory subsystem owns plain storage regions such as WRAM and HRAM; it must not bypass bus-visible access restrictions defined elsewhere.
- Shared scheduling must allow CPU, DMA, PPU, timer, and other actors to make progress on the same T-cycle timeline so arbitration remains observable.

## Boot ROM architecture policy

- Treat boot ROM as firmware executed by the real CPU model, not as a fake initialization script.
- Keep DMG-family hardware separate from boot ROM assets: one hardware core, multiple selectable boot ROM images.
- Boot ROM selection should depend on the console model and support at least real boot ROM execution, custom boot ROM injection, and direct boot without firmware.
- Direct-boot helpers are a testing and tooling feature, not a replacement for real boot ROM execution.
- The boot subsystem should not assume every model uses the same boot firmware size or address mapping layout; keep those details inside the boot and bus design, not spread through unrelated subsystems.

## Portability policy

- No platform-specific APIs inside the emulation core.
- Keep file I/O, audio output, video output, and input outside the core.
- Use traits or narrow interfaces where frontend services must be injected.
- The same core should be usable by CLI tools, desktop apps, benchmarks, tests, and WebAssembly.

## Scalability policy

- New hardware quirks must be added behind well-defined subsystem boundaries.
- Avoid spreading model checks across unrelated modules.
- Centralize model and revision capabilities.
- Do not couple DMG-only shortcuts into APIs that would block CGB banking, palettes, HDMA, or double speed later.
- Prefer capability-driven branching from a shared model description over ad hoc per-subsystem variant checks.
- Prefer bus-side dynamic mapping and access-state rules over flattening everything into static memory ownership tables.
