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

## Suggested subsystem boundaries

- CPU: instruction flow, register state, interrupt acceptance, HALT/STOP semantics
- Bus: address decoding, subsystem routing, visible access ordering
- Memory and MMIO: WRAM, HRAM, register ownership, access restrictions
- Interrupt controller: IF/IE state and request/acknowledge flow
- Timer: DIV/TIMA/TMA/TAC behavior and edge-sensitive increment logic
- PPU: LCD modes, fetcher/FIFO behavior, rendering state, VRAM/OAM restrictions
- DMA: OAM DMA and future HDMA scheduling and blocking rules
- APU: internal channel/frame-sequencer state only, not output backends
- Joypad and serial: hardware-visible registers and signaling
- Cartridge and MBC: ROM/RAM banking, RTC, rumble, and mapper-specific behavior
- Boot ROM and model config: power-up state, revision differences, direct-boot setup
- Model-specific extensions: CGB and later SGB

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
