# Phase 2 ROM targets

These filenames are reserved for the first `gb-test-runner` Phase 2 automation
targets.

The runner is still contract-only, so the files are not shipped yet. The typed
suite metadata in `gb-test-runner` is the source of truth for the intended CPU
and interrupt timing targets:

- `phase2_fetch_immediate_order.gb`
- `phase2_control_flow_stack_cb.gb`
- `phase2_ei_delay_priority.gb`
- `phase2_halt_stop_and_halt_bug.gb`
- `phase2_timer_if_visibility_and_service.gb`

Prefer tiny synthetic ROMs that produce trace-based pass conditions first.
