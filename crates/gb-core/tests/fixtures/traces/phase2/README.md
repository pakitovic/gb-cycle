# Phase 2 trace targets

These filenames are reserved for the first `gb-test-runner` Phase 2 CPU and
interrupt timing automation targets.

All current Phase 2 synthetic trace targets are now shipped as golden text
traces. The typed suite metadata in `gb-test-runner` remains the source of
truth for the intended trace artifacts:

- `phase2_fetch_immediate_order.trace` (`shipped`)
- `phase2_control_flow_stack_cb.trace` (`shipped`)
- `phase2_ei_delay_priority.trace` (`shipped`)
- `phase2_halt_stop_and_halt_bug.trace` (`shipped`)
- `phase2_timer_if_visibility_and_service.trace` (`shipped`)

Prefer short text traces that pinpoint the first timing divergence rather than
large end-of-test dumps. The shipped traces are full golden machine traces
generated from the reproducible builders in `crates/gb-core/tests/phase2.rs`.
The `halt_stop_and_halt_bug` case now also has one explicit timed joypad wake
recorded in `gb-test-runner` metadata at `t_cycle = 380`, so the external
stimulus is part of the typed test contract rather than an undocumented local
detail.
