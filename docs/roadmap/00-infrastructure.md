# Phase 0 — Verification, debugging, and base architecture infrastructure

1. **Test model and test ROM strategy**
2. **Debugging infrastructure, tracing, and internal tools**
3. **General DMG model and architectural preparation for CGB**

#### Goal

Establish the project's methodological and architectural foundation before locking down detailed hardware behavior.

This phase must define:

- the global temporal model of the core
- the module structure and responsibilities
- the test strategy
- the minimum observability and debugging infrastructure
- the base design that allows future growth without blocking a later CGB expansion

#### Modules involved

- `model/`
- `scheduler/`
- `debugger/`
- `gb-test-runner/`
- `tests/`
- `lib.rs`

#### Deliverables

##### Tests and validation strategy

- initial unit and integration test structure
- test ROM strategy
- convention for golden traces and expected outputs
- classification of suites by subsystem
- reusable runners or helpers from `gb-test-runner/`

##### Debugging and tracing

- base infrastructure in `debugger/`
- initial trace format
- per-subsystem trace hooks
- core state snapshots
- inspection points connectable to the scheduler
- foundation for breakpoints and watchpoints

##### Base architecture

- base hardware types in `model/`
- T-cycle scheduler skeleton
- explicit per-T-cycle phase order with one `step_t_cycle()`-style top-level entry point
- definition of responsibilities by module
- initial interfaces between CPU, bus, PPU, DMA, timer, cartridge, and debugger
- a cycle-local context shape carrying external events, derived signals, ownership facts, and queued side effects or IRQ requests
- conventions to avoid mixing frontend logic with core logic

#### Done criteria

- there is a clear document or convention for the test strategy
- the project can run base tests against `gb-core`
- there is a reusable minimum tracing infrastructure
- the scheduler has a defined notion of T-cycle advancement
- the scheduler phase order is explicit in code and docs rather than implicit in subsystem call chains
- the responsibility split between modules is fixed
- cycle traces can expose enough per-T-cycle state to debug scheduler ordering issues
- the core does not depend on `gb-cli`, `gb-desktop`, or `gb-web` to function
- the architecture is prepared to incorporate CGB without contaminating DMG behavior yet

#### Risks if omitted or overly simplified

- massive rework when introducing tracing later
- the need to rewrite CPU or PPU just to inspect them properly
- tests that are not useful for debugging fine timing issues
- incorrect coupling between frontend and core
- DMG decisions that are too rigid and make a future CGB extension harder

Boundary note: Phase `0` fixes architecture, tracing, and the scheduler
skeleton. Phase `1` is where that skeleton becomes hardware-visible stepping,
arbitration, MMIO, and startup behavior.

Status note (`2026-03-15`): Phase `0` baseline is closed in the current repo.
The project now has the documented test layout, `gb-test-runner` typed
ROM-harness crate, typed debugger breakpoints/watchpoints, `Machine` plus
`step_t_cycle()`, explicit scheduler phases, stubbed subsystem boundaries,
typed debug snapshots, and deterministic scheduler-aligned subsystem trace
hooks. Remaining work moves to Phase `1`.

