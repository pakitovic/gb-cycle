# Phase 4 trace fixtures

This directory contains golden text traces paired with the Phase `4` synthetic OAM corruption ROM fixtures, including console-model variants for the `inc hl` trigger family.

Current fixtures:

- `phase4_oam_bug_direct_mode2_oam_access.trace`
- `phase4_oam_bug_fea0_mode2_read.trace`
- `phase4_oam_bug_inc_hl_dmg0.trace`
- `phase4_oam_bug_inc_hl_dmg.trace`
- `phase4_oam_bug_inc_hl_mgb.trace`
- `phase4_oam_bug_hli_hld.trace`
- `phase4_oam_bug_stack_and_interrupt_service.trace`
- `phase4_oam_bug_inc_hl_cgb.trace`

`crates/gb-core/tests/phase4.rs` owns the trace-producing scenarios and expected OAM outcomes. These full golden machine traces lock trigger timing and scheduler chronology together for the current synthetic scenarios.

Regenerate the trace fixtures and paired ROM fixtures with:

`GB_CYCLE_ACCEPT_PHASE4_FIXTURES=1 cargo test -q -p gb-core --test phase4 -- --test-threads=1`
