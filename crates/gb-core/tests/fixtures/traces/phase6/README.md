# Phase 6 trace fixtures

Retained synthetic cartridge traces for shipped Phase `6` ROM fixtures:

- `phase6_mbc1_standard_banking.trace`
- `phase6_mbc1_small_rom_mask_and_ram.trace`
- `phase6_mbc2_control_decode_and_nibble_ram.trace`
- `phase6_mbc3_banking_ram_and_rtc.trace`
- `phase6_mbc5_rom_banking_rumble_and_ram.trace`

Regenerate with:

`GB_CYCLE_ACCEPT_PHASE6_FIXTURES=1 cargo test -q -p gb-core --test phase6 -- --test-threads=1`
