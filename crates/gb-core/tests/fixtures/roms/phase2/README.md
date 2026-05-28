# Phase 2 ROM fixtures

This directory contains shipped synthetic `NoMBC` ROM fixtures for the Phase `2` CPU, interrupt, timer, `HALT`, `STOP`, and HALT-bug timing targets consumed by `gb-core` tests and the built-in `gb-test-runner` Phase `2` suite.

Current fixtures:

- `phase2_fetch_immediate_order.gb`
- `phase2_control_flow_stack_cb.gb`
- `phase2_ei_delay_priority.gb`
- `phase2_halt_stop_and_halt_bug.gb`
- `phase2_timer_if_visibility_and_service.gb`

`crates/gb-core/tests/phase2.rs` owns the Rust builders and expected end states. Regenerate the ROM fixtures and paired trace fixtures with:

`GB_CYCLE_ACCEPT_PHASE2_FIXTURES=1 cargo test -q -p gb-core --test phase2 -- --test-threads=1`
