# Phase 2 ROM targets

These filenames are reserved for the first `gb-test-runner` Phase 2 automation
targets.

All current Phase 2 synthetic ROM targets are now shipped as locally generated
`NoMBC` fixtures. The typed suite metadata in `gb-test-runner` remains the
source of truth for the intended CPU and interrupt timing targets:

- `phase2_fetch_immediate_order.gb` (`shipped`)
- `phase2_control_flow_stack_cb.gb` (`shipped`)
- `phase2_ei_delay_priority.gb` (`shipped`)
- `phase2_halt_stop_and_halt_bug.gb` (`shipped`)
- `phase2_timer_if_visibility_and_service.gb` (`shipped`)

Prefer tiny synthetic ROMs that produce trace-based pass conditions first.
`crates/gb-core/tests/phase2.rs` is the source of truth for the shipped
builders and their expected end states.
