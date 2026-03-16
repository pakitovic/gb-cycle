# SameBoy

Repository: https://github.com/LIJI32/SameBoy

## Why keep this reference
- Strong global DMG/CGB reference
- Practical ceiling reference for timing-sensitive DMG/CGB behavior, especially LCD/PPU work
- Very useful for PPU/LCD timing and APU
- Use as an open-source oracle and implementation reference

## What to consult it for
- Architecture ideas
- Subsystem-specific implementation patterns
- Behavioral cross-checks when documentation is ambiguous

## Cautions
- Do not treat implementation details as hardware truth without external support

## Typical subsystems to inspect
- CPU / bus
- PPU / LCD
- Timing-sensitive behavior

## Notes for this repo
- Record here the files, modules, or patterns that become relevant during implementation.

## Phase 2.8 source-level cross-check (`2026-03-16`)

This repo's current Phase `2` closure uses a source-level SameBoy comparison,
not yet an automated first-divergence runner. The goal here was to cross-check
the specific timing-sensitive choices landed in Phase `2`, while keeping the
full differential tooling explicitly deferred to Phase `9`.

- `Core/timing.c`
  - `TAC_TRIGGER_BITS = {512, 8, 32, 128}` matches the DMG timer-bit selection
    baseline used in this repo.
  - `GB_set_internal_div_counter()` advances TIMA from a falling-edge style
    trigger (`div_counter & ~value`), which matches the repo's edge-driven
    timer model rather than an accumulator-based approximation.
  - SameBoy keeps timer glitch handling explicit in `GB_emulate_timer_glitch()`,
    which matches the repo choice to model `DIV` / `TAC` glitch-triggered TIMA
    increments as first-class behavior.
  - SameBoy also keeps timer request visibility separate from the initial
    overflow step through `tima_reload_state`, where the `IF` timer bit is set
    on a later state-machine advance rather than collapsed into the first
    overflow increment.
- `Core/memory.c`
  - Boot ROM reads stay active only while `!boot_rom_finished`, and `GB_IO_BANK`
    write handling uses `gb->boot_rom_finished |= value & 1`. That matches the
    repo's `FF50` contract that a non-zero write disables the boot overlay and
    that the next relevant fetch should see cartridge mapping.
- `Core/sm83_cpu.c`
  - SameBoy documents `DI` as not delayed and `EI` as "disable interrupts for
    one instruction, then enable them", which matches the repo's delayed-`EI`
    and immediate-`DI` contract.
  - `RETI` restores control flow and then re-enables `IME` immediately, which
    matches the repo's explicit `RETI` behavior instead of routing it through
    delayed `EI`.
  - Control-flow helpers such as `CALL`, `RST`, and `RET` still push and pop the
    stack bytewise through distinct `cycle_write` / `cycle_read` operations,
    which matches the repo's bytewise stack and interrupt-service model.

Relevant SameBoy source entry points:

- https://github.com/LIJI32/SameBoy/blob/master/Core/timing.c
- https://github.com/LIJI32/SameBoy/blob/master/Core/memory.c
- https://github.com/LIJI32/SameBoy/blob/master/Core/sm83_cpu.c
