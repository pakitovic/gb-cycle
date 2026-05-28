# Phase 5 trace fixtures

This directory contains golden text traces for Phase `5` input and serial closure.

Current fixtures:

- `phase5_joypad_stop_wake_and_irq.trace`
- `phase5_serial_external_clock_progress.trace`

`crates/gb-core/tests/phase5.rs` owns the trace-producing scenarios. Regenerate the trace fixtures with:

`GB_CYCLE_ACCEPT_PHASE5_FIXTURES=1 cargo test -q -p gb-core --test phase5 -- --test-threads=1`
