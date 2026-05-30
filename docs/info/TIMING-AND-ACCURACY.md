# Timing and Accuracy

This document owns shared timing vocabulary, accuracy-claim policy, and the project-level scheduler contract. Subsystem-specific behavior lives in the matching `docs/hardware/` handbook; crate/module ownership lives in [`../ARCHITECTURE.md`](../ARCHITECTURE.md); validation policy lives in [`../TESTING.md`](../TESTING.md); ROM-suite operation lives in [`ROM-SUITES.md`](ROM-SUITES.md); source consultation order lives in [`../REFERENCES.md`](../REFERENCES.md).

## Accuracy claim policy

Do not use "cycle accurate" as a blanket project label. Make claims at the narrowest supported scope: CPU instruction timing, bus arbitration, timer edge behavior, interrupt ordering, PPU dot/fetcher/FIFO behavior, DMA timing, serial/link timing, APU sequencing, startup/boot handoff, cartridge side effects, save-state continuation, or a specific ROM-suite lane.

`Strict` evidence is the only evidence that supports official accuracy, CI, closure, and promotion claims. `Permissive` and `Experimental` results may be useful for compatibility or research, but they must stay labeled as such and must not be used as strict accuracy evidence.

A timing claim should state what was validated, where the behavior is owned, which evidence supports it, and what remains unverified. A final framebuffer, a game booting, or a matching aggregate instruction count is not enough for timing-sensitive behavior if the path to that output can still hide wrong ordering.

## Evidence order and current resources

Use this order unless a subsystem handbook narrows it with a stronger source:

1. Real hardware evidence, official/manual documentation, and hardware research listed in [`../REFERENCES.md`](../REFERENCES.md), especially Pan Docs, AntonioND, and Gekkio.
2. The owning local handbook under `docs/hardware/`, plus shared architecture and testing policy in [`../ARCHITECTURE.md`](../ARCHITECTURE.md), [`../TESTING.md`](../TESTING.md), and this file.
3. Executable evidence from typed tests, manifests, GBEmulatorShootout-sourced rows, DocBoy rows, linked-session manifests, retained traces, snapshots, and report channels documented in [`ROM-SUITES.md`](ROM-SUITES.md).
4. Open-source emulator source or differential comparison when it provides comparable observables, without treating implementation behavior as hardware truth.
5. Project-local assumptions, which must be documented as assumptions or TODOs rather than promoted to hardware fact.

Current executable resources include promoted DMG/SGB, promoted CGB, green extra/internal, large DocBoy, RealBoot-local, private-manifest, and linked-session lanes. Keep their report separation intact: `/test/gb-emulator-shootout/test-report.md` for promoted GB Emulator Shootout rows, `/test/test-report-extra.md` for green non-DocBoy extra/internal rows, `/test/docboy/test-report.md` for large DocBoy single-machine rows, and stdout/artifacts for linked-session rows.

## Timing vocabulary

- `T-cycle` is the fundamental project timing unit for powered-on core execution.
- `M-cycle` is a descriptive grouping of four T-cycles, not the scheduler granularity.
- `dot` is the LCD/PPU scan-domain unit; on DMG-family normal-speed execution it aligns with T-cycle reasoning, while CGB double speed separates CPU-visible scheduler cadence from LCD-domain dot cadence.
- `RealBoot`, `SkipBoot`, and `CustomBoot` are startup modes with different evidence value; boot-ROM bytes are private/local and selected through model/revision policy rather than a repo-hosted firmware path.
- `Strict`, `Permissive`, and `Experimental` are execution/evidence modes around the same hardware model, not alternate timing models for already-supported hardware.

When external documentation expresses timing in M-cycles, dots, frames, hertz, or microseconds, restate the behaviorally relevant value in T-cycles or in an explicit domain relationship before encoding it in code, tests, or deferred work.

## Shared timeline model

CPU, PPU, DMA, timer, APU, serial, joypad, bus, interrupt, boot, cartridge, and link-visible effects must be explainable on one deterministic shared timeline. Avoid whole-instruction, whole-M-cycle, frame callback, host-thread, or audio-callback designs that advance hardware after the fact and then patch visible state.

Host work is outside the emulated T-cycle timeline. File I/O, save flushing, atomic replacement, UI events, audio device callbacks, and wall-clock sleeps must not reorder already-committed hardware-visible state. Battery-backed off-session behavior such as MBC3 RTC advance may use an explicit elapsed-time source at the persistence boundary, but powered-on bus-visible behavior still needs a T-cycle-domain contract when it matters.

CGB double speed extends the same model instead of creating a second emulator core. The scheduler T-cycle remains CPU-visible; the LCD/video domain keeps its own cadence, so a full double-speed CPU-visible frame can span `140448` scheduler T-cycles while the LCD domain still advances `70224` dots by gating video work to every other scheduler T-cycle. Do not model double speed as a generic multiplier for every subsystem.

## Global scheduler contract

The preferred shape is one deterministic scheduler, such as `GlobalScheduler` plus a `step_t_cycle()`-style entry point, or an equally explicit equivalent. The scheduler coordinates ordering, cycle-local context, trace points, and synchronization; it must not reimplement subsystem-owned quirks.

Observable per-T-cycle phase order:

1. external event ingress
2. master clock / shared system-counter tick
3. free-running counter-derived edge resolution
4. autonomous peripheral ticks
5. bus arbitration for the current T-cycle
6. CPU micro-operation
7. MMIO side-effect commit
8. interrupt aggregation into `IF`
9. CPU wake / interrupt-accept evaluation

This order is an architectural contract for visible dependencies, not a claim that Nintendo published one canonical internal scheduler. Another implementation shape is acceptable only if it preserves the same observable ordering for PPU mode visibility, DMA blocking, timer overflow delay, serial completion timing, joypad visible-edge IRQs, MMIO visibility, same-cycle timer queued-request opcode preemption, and CPU interrupt acceptance.

Free-running divider-derived events such as timer input edges and `DIV-APU` edge detection belong after the shared counter advances and before autonomous peripherals consume those edges. Immediate MMIO effects produced by a write on the access T-cycle, such as `DIV` reset, `FF46` DMA start, `SC.7` transfer start, `LCDC.7` transitions, or `FF50` unmapping, belong to the owning device when the access commits.

Hardware interrupt sources publish into the interrupt controller during aggregation; CPU acceptance is a later CPU-owned decision. Keep those contracts separate even when same-cycle behavior needs a documented exception, such as timer reload requests already queued before the CPU opcode slot.

## Subsystem timing expectations

- CPU execution should remain an in-flight fetch/decode/execute model made of ordered opcode fetch, immediate fetch, memory read/write, stack, branch, interrupt-service, `HALT`, `STOP`, and internal no-bus steps.
- Bus-visible access policy must be coherent with the same current-cycle state that software can read through MMIO, including `STAT`, `LY`, VRAM/OAM accessibility, DMA blocking, timer/interrupt state, serial progress, and joypad visibility.
- Timer behavior should derive visible events from clocked internal state transitions, including divider/TAC edge cases and the delayed timer reload/IRQ path, rather than from coarse accumulated-period shortcuts.
- APU timing should derive frame-sequencer, `DIV-APU`, channel timers, mixer-visible DAC state, and HPF state from the shared timeline; host sample production observes hardware state and must not drive it.
- PPU behavior should be dot-by-dot enough that mode visibility, fetcher/FIFO behavior, register snapshot timing, and bus restrictions can be tested against the same timeline.
- DMA should expose in-flight transfer state, CPU/bus impact, source/destination visibility, and byte/block commit timing separately enough to support OAM DMA, GDMA, HDMA, and future windowed transfers.
- Serial and linked-session timing should enter external clocks, peer bits, cable routing, DMG-07 adapter behavior, and CGB IR optical routing through explicit ingress/topology seams, not frontend sleeps or serial-local shortcuts.
- Joypad input should enter as hardware-facing button transitions on the shared timeline; visible `P1` low-nibble transitions, interrupt requests, and `STOP` wake must stay ordered.
- Boot/startup timing should distinguish RealBoot firmware execution, skip/custom direct-start state synthesis, `FF50` handoff, and model/revision-specific hidden-state seeds.
- Cartridge timing should keep powered-on mapper side effects on the shared timeline and keep off-session persistence advancement explicit at the persistence boundary.

## Validation and trace policy

Validate timing-sensitive work with the narrowest useful unit/integration tests first, then ROM suites, retained artifacts, report comparisons, or external manual differentials when they add evidence. For known external-ROM failures or timing-sensitive reruns, follow the before/after report workflow from [`../TESTING.md`](../TESTING.md) and [`ROM-SUITES.md`](ROM-SUITES.md).

Traces, snapshots, debug instrumentation, and failure artifacts must preserve the hardware-visible T-cycle behavior they explain. Prefer first-divergence evidence over final-frame summaries when diagnosing scheduler, bus, PPU, timer, interrupt, serial, joypad, DMA, APU, startup, or linked-session regressions.

Determinism and save/load continuation are timing evidence. Replays, save states, rewind restore paths, startup overrides, execution modes, external stimuli, injected clocks, and ROM fingerprints must be explicit enough that two runs with the same inputs reproduce the same observable timeline.

## Claim checklist

Before calling timing work closed, be able to answer:

- Which subsystem or cross-subsystem contract owns the behavior?
- Is the claim expressed in T-cycles, dots, or an explicit cross-domain relationship?
- Which strict tests, ROM rows, reports, traces, snapshots, or external comparisons support it?
- Are `Permissive`, `Experimental`, DocBoy-extra, RealBoot-local, private-manifest, or other non-strict results clearly labeled and kept out of strict closure claims?
- Does the implementation preserve the scheduler phase contract and owner boundaries instead of hiding timing in convenience batching?
- Are remaining assumptions or gaps recorded in [`../TODO.md`](../TODO.md) or the owning hardware handbook?
