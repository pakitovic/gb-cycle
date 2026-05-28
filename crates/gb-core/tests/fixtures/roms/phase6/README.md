# Phase 6 ROM fixtures

This directory contains shipped synthetic cartridge ROM fixtures for Phase `6` mapper behavior covering `MBC1`, `MBC2`, `MBC3`, `MBC5`, and dedicated `MBC6` behavior.

Current fixtures:

- `phase6_mbc1_standard_banking.gb`
- `phase6_mbc1_small_rom_mask_and_ram.gb`
- `phase6_mbc2_control_decode_and_nibble_ram.gb`
- `phase6_mbc3_banking_ram_and_rtc.gb`
- `phase6_mbc5_rom_banking_rumble_and_ram.gb`
- `phase6_mbc6_split_window_flash.gb`

The builders in `phase6.rs` and `synthetic_cartridge.rs` are the source of truth for regenerating these fixtures. Regenerate the ROM fixtures with:

`GB_CYCLE_ACCEPT_PHASE6_FIXTURES=1 cargo test -q -p gb-core --test phase6 -- --test-threads=1`
