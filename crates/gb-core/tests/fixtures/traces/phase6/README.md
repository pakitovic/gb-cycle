# Phase 6 trace fixtures

This directory contains retained synthetic cartridge traces for the Phase `6` `MBC1`, `MBC2`, `MBC3`, and `MBC5` oracle cases.

Current fixtures:

- `phase6_mbc1_standard_banking.trace`
- `phase6_mbc1_small_rom_mask_and_ram.trace`
- `phase6_mbc2_control_decode_and_nibble_ram.trace`
- `phase6_mbc3_banking_ram_and_rtc.trace`
- `phase6_mbc5_rom_banking_rumble_and_ram.trace`

There is no `MBC6` trace in this directory because `MBC6` is covered by automated cargo tests for the dedicated synthetic split-window/flash fixture rather than by the shared Phase `6` cartridge trace set.

`crates/gb-core/tests/phase6.rs` owns the synthetic ROM builders and can regenerate the ROM fixtures. If these retained trace artifacts are intentionally refreshed, document the generator or runner path in the same change instead of relying on an implicit command.
