# Phase 4 ROM targets

These filenames are reserved for the first `gb-test-runner` Phase 4 PPU
automation targets covering DMG-family OAM corruption validation.

All current synthetic ROM targets are now shipped as locally generated `NoMBC`
fixtures. The typed suite metadata in `gb-test-runner` remains the source of
truth for the target list:

- `phase4_oam_bug_direct_mode2_oam_access.gb` (`shipped`)
- `phase4_oam_bug_fea0_mode2_read.gb` (`shipped`)
- `phase4_oam_bug_inc_hl.gb` (`shipped`)
- `phase4_oam_bug_hli_hld.gb` (`shipped`)
- `phase4_oam_bug_stack_and_interrupt_service.gb` (`shipped`)

Prefer tiny synthetic ROMs that isolate one trigger family or hardware-model
gating path at a time. Keep the Rust-side program builder close to the
consuming integration test so the checked-in binary fixture stays reproducible.
`crates/gb-core/tests/phase4.rs` is the source of truth for the shipped
builders and expected OAM outcomes.
