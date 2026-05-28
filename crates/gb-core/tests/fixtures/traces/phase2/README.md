# Phase 2 trace fixtures

This directory contains golden text traces paired with the Phase `2` synthetic CPU, interrupt, timer, `HALT`, `STOP`, and HALT-bug ROM fixtures.

Current fixtures:

- `phase2_fetch_immediate_order.trace`
- `phase2_control_flow_stack_cb.trace`
- `phase2_ei_delay_priority.trace`
- `phase2_halt_stop_and_halt_bug.trace`
- `phase2_timer_if_visibility_and_service.trace`

`crates/gb-core/tests/phase2.rs` owns the trace-producing scenarios. The `halt_stop_and_halt_bug` case includes one explicit timed joypad wake recorded in `gb-test-runner` metadata at `t_cycle = 380`, so the external stimulus is part of the typed test contract rather than an undocumented local detail.

Regenerate the trace fixtures and paired ROM fixtures with:

`GB_CYCLE_ACCEPT_PHASE2_FIXTURES=1 cargo test -q -p gb-core --test phase2 -- --test-threads=1`
