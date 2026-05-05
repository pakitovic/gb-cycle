# Phase 6 ROM targets

These filenames are reserved for the first Phase `6` synthetic cartridge fixtures covering shipped `MBC1`, `MBC2`, `MBC3`, `MBC5`, and dedicated `MBC6` behavior.

Current shipped targets:

- `phase6_mbc1_standard_banking.gb`
- `phase6_mbc1_small_rom_mask_and_ram.gb`
- `phase6_mbc2_control_decode_and_nibble_ram.gb`
- `phase6_mbc3_banking_ram_and_rtc.gb`
- `phase6_mbc5_rom_banking_rumble_and_ram.gb`
- `phase6_mbc6_split_window_flash.gb`

The builders in [phase6.rs](../../../phase6.rs) and [synthetic_cartridge.rs](../../../common/synthetic_cartridge.rs) are the source of truth for regenerating these fixtures.
