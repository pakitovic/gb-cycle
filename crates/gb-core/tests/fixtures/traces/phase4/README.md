# Phase 4 trace targets

These filenames are reserved for the first `gb-test-runner` Phase 4 PPU OAM
corruption automation targets.

All current synthetic trace targets are now shipped as golden text traces. The
typed suite metadata in `gb-test-runner` remains the source of truth for the
target list:

- `phase4_oam_bug_direct_mode2_oam_access.trace` (`shipped`)
- `phase4_oam_bug_fea0_mode2_read.trace` (`shipped`)
- `phase4_oam_bug_inc_hl_dmg0.trace` (`shipped`)
- `phase4_oam_bug_inc_hl_dmg.trace` (`shipped`)
- `phase4_oam_bug_inc_hl_mgb.trace` (`shipped`)
- `phase4_oam_bug_hli_hld.trace` (`shipped`)
- `phase4_oam_bug_stack_and_interrupt_service.trace` (`shipped`)
- `phase4_oam_bug_inc_hl_cgb.trace` (`shipped`)

Prefer short trusted traces or oracle-derived excerpts grouped by trigger
family and console model. The shipped traces are intentionally full golden
machine traces generated from the reproducible builders in
`crates/gb-core/tests/phase4.rs`, so each asset locks trigger timing and
scheduler chronology together for its current synthetic scenario.
