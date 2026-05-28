# Phase 4 ROM fixtures

This directory contains shipped synthetic `NoMBC` ROM fixtures for the Phase `4` DMG-family OAM corruption targets consumed by `gb-core` tests and the built-in `gb-test-runner` Phase `4` suite.

Current fixtures:

- `phase4_oam_bug_direct_mode2_oam_access.gb`
- `phase4_oam_bug_fea0_mode2_read.gb`
- `phase4_oam_bug_inc_hl.gb`
- `phase4_oam_bug_hli_hld.gb`
- `phase4_oam_bug_stack_and_interrupt_service.gb`

`crates/gb-core/tests/phase4.rs` owns the Rust builders and expected OAM outcomes. Regenerate the ROM fixtures and paired trace fixtures with:

`GB_CYCLE_ACCEPT_PHASE4_FIXTURES=1 cargo test -q -p gb-core --test phase4 -- --test-threads=1`
