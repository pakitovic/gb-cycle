# Architecture

## Goals
- Prioritize hardware-accurate behavior.
- Keep the core modular enough to evolve from DMG-first to CGB and later SGB support.
- Keep portability high: core logic must remain platform-agnostic.

## Recommended high-level layout
- `core/`: pure emulation logic with no frontend dependencies.
- `frontends/`: CLI, desktop, web, or test harnesses.
- `tests/`: ROM-based tests, golden data, integration helpers.
- `docs/` or `AI/`: design and research docs.

## Core design principles
- Model hardware by responsibilities, not by UI-facing features.
- Favor explicit state transitions over implicit side effects.
- Keep timing ownership clear.
- Separate behavior specification from optimization strategies.

## Suggested subsystem boundaries
- CPU
- Bus
- Memory map / MMIO
- Interrupt controller
- Timer
- PPU
- DMA
- APU
- Joypad
- Serial
- Cartridge / MBC
- Boot ROM / model configuration
- Model-specific extensions: CGB, SGB

## Portability policy
- No platform-specific APIs inside the emulation core.
- Keep file I/O, audio output, video output, and input outside the core.
- Use traits or narrow interfaces where frontend services must be injected.

## Scalability policy
- New hardware quirks must be added behind well-defined subsystem boundaries.
- Avoid spreading model checks across unrelated modules.
- Centralize model/revision capabilities.
