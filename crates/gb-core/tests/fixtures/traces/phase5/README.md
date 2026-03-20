# Phase 5 trace fixtures

Retained timing artifacts for input and serial closure:

- `phase5_joypad_stop_wake_and_irq.trace`
- `phase5_serial_external_clock_progress.trace`

Regenerate with:

`GB_CYCLE_ACCEPT_PHASE5_FIXTURES=1 cargo test -q -p gb-core --test phase5 -- --test-threads=1`
