use super::*;

fn loaded_mbc6() -> Mbc6Cartridge {
    let report = CartridgeSlot::load(build_banked_mbc6_rom(), &CompatibilityPolicy::strict())
        .expect("MBC6 should load");
    let Some(CartridgeDevice::Mbc6(cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC6 cartridge");
    };
    cartridge
}

fn select_flash_window_a(cartridge: &mut Mbc6Cartridge) {
    cartridge.write_rom(0x0C00, 0x01);
    cartridge.write_rom(0x2800, 0x08);
    cartridge.write_rom(0x2000, 0x02);
}

fn select_flash_window_b(cartridge: &mut Mbc6Cartridge) {
    cartridge.write_rom(0x0C00, 0x01);
    cartridge.write_rom(0x3800, 0x08);
    cartridge.write_rom(0x3000, 0x02);
}

fn flash_unlock(cartridge: &mut Mbc6Cartridge) {
    cartridge.write_rom(0x2000, 0x02);
    cartridge.write_rom(0x5555, 0xAA);
    cartridge.write_rom(0x2000, 0x01);
    cartridge.write_rom(0x4AAA, 0x55);
    cartridge.write_rom(0x2000, 0x02);
}

fn flash_unlock_b(cartridge: &mut Mbc6Cartridge) {
    cartridge.write_rom(0x3000, 0x02);
    cartridge.write_rom(0x7555, 0xAA);
    cartridge.write_rom(0x3000, 0x01);
    cartridge.write_rom(0x6AAA, 0x55);
    cartridge.write_rom(0x3000, 0x02);
}

fn flash_program_command(cartridge: &mut Mbc6Cartridge) {
    flash_unlock(cartridge);
    cartridge.write_rom(0x5555, 0xA0);
}

fn flash_erase_command(cartridge: &mut Mbc6Cartridge) {
    flash_unlock(cartridge);
    cartridge.write_rom(0x5555, 0x80);
    flash_unlock(cartridge);
}

fn flash_extended_command(cartridge: &mut Mbc6Cartridge, command: u8) {
    flash_unlock(cartridge);
    cartridge.write_rom(0x5555, 0x60);
    flash_unlock(cartridge);
    cartridge.write_rom(0x5555, command);
}

fn flash_hidden_read_command(cartridge: &mut Mbc6Cartridge) {
    flash_unlock(cartridge);
    cartridge.write_rom(0x5555, 0x77);
    flash_unlock(cartridge);
    cartridge.write_rom(0x5555, 0x77);
}

fn reset_flash(cartridge: &mut Mbc6Cartridge) {
    cartridge.write_rom(0x4000, 0xF0);
}

fn commit_program_block_with_first_byte(
    cartridge: &mut Mbc6Cartridge,
    bank: u8,
    first_byte: u8,
    final_byte: u8,
) {
    cartridge.write_rom(0x2000, bank);
    cartridge.write_rom(0x4000, first_byte);
    cartridge.write_rom(0x407F, final_byte);
    cartridge.write_rom(0x407F, final_byte);
    assert_eq!(cartridge.read_rom(0x4000), 0x80);
    cartridge.write_rom(0x4000, 0xF0);
}

#[test]
fn mbc6_loads_as_a_dedicated_supported_family_with_documented_power_up_banks() {
    let report = CartridgeSlot::load(build_banked_mbc6_rom(), &CompatibilityPolicy::strict())
        .expect("MBC6 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc6);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc6)
    );

    let Some(CartridgeDevice::Mbc6(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC6 cartridge");
    };
    assert_eq!(
        cartridge.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: MBC6_SUPPORTED_RAM_BYTES,
                },
                flash_byte_len: MBC6_FLASH_BYTES,
                hidden_byte_len: MBC6_HIDDEN_BYTES,
            },
        }
    );
    assert!(!cartridge.ram_enabled);
    assert!(!cartridge.flash_enabled);
    assert!(!cartridge.flash_write_enabled);
    assert_eq!(cartridge.ram_bank_a, 0);
    assert_eq!(cartridge.ram_bank_b, 1);
    assert_eq!(cartridge.rom_flash_bank_a, 2);
    assert_eq!(cartridge.rom_flash_bank_b, 3);
    assert_eq!(cartridge.window_select_a, Mbc6WindowSelect::Rom);
    assert_eq!(cartridge.window_select_b, Mbc6WindowSelect::Rom);
    assert_eq!(cartridge.read_rom(0x0000), 0x00);
    assert_eq!(cartridge.read_rom(0x2000), 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x02);
    assert_eq!(cartridge.read_rom(0x6000), 0x03);
}

#[test]
fn mbc6_strict_validation_requires_the_official_net_de_get_shape() {
    let mut non_cgb = build_banked_mbc6_rom();
    non_cgb[CGB_FLAG_ADDRESS] = 0x00;
    let non_cgb_error = CartridgeSlot::load(non_cgb, &CompatibilityPolicy::strict())
        .expect_err("MBC6 without CGB capability should fail strict validation");
    assert!(matches!(
        non_cgb_error,
        CartridgeLoadError::Rejected { reason, .. } if reason.contains("CGB-capable")
    ));

    let mut unknown_cgb = build_banked_mbc6_rom();
    unknown_cgb[CGB_FLAG_ADDRESS] = 0x42;
    let unknown_cgb_error = CartridgeSlot::load(unknown_cgb, &CompatibilityPolicy::strict())
        .expect_err("MBC6 with a non-CGB-capable unknown CGB flag should fail strict validation");
    assert!(matches!(
        unknown_cgb_error,
        CartridgeLoadError::Rejected { reason, .. } if reason.contains("CGB-capable")
    ));

    let short_rom = build_test_rom(512 * 1024, 0x20, 0x04, 0x03);
    let short_error = CartridgeSlot::load(short_rom, &CompatibilityPolicy::strict())
        .expect_err("MBC6 with the wrong ROM size should fail");
    assert!(matches!(
        short_error,
        CartridgeLoadError::Rejected { reason, .. } if reason.contains("1 MiB ROM")
    ));

    let wrong_ram = build_test_rom(MBC6_SUPPORTED_ROM_BYTES, 0x20, 0x05, 0x02);
    let ram_error = CartridgeSlot::load(wrong_ram, &CompatibilityPolicy::strict())
        .expect_err("MBC6 with the wrong SRAM size should fail");
    assert!(matches!(
        ram_error,
        CartridgeLoadError::Rejected { reason, .. } if reason.contains("32 KiB SRAM")
    ));
}

#[test]
fn mbc6_banks_rom_in_independent_8kib_windows_without_zero_bank_translation() {
    let mut cartridge = loaded_mbc6();

    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x3000, 0x7F);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);
    assert_eq!(cartridge.read_rom(0x6000), 0x7F);

    cartridge.write_rom(0x2000, 0x80);
    cartridge.write_rom(0x3000, 0xFF);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);
    assert_eq!(cartridge.read_rom(0x6000), 0x7F);
}

#[test]
fn mbc6_exposes_two_independent_4kib_sram_windows() {
    let mut cartridge = loaded_mbc6();
    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    let disabled_a = cartridge.describe_external_access(0xA000);
    assert_eq!(
        disabled_a.target(),
        CartridgeExternalTarget::Mbc6Ram {
            window: Mbc6Window::A,
            bank: 0,
        }
    );
    assert_eq!(
        disabled_a.availability(),
        CartridgeExternalAvailability::Disabled
    );

    cartridge.write_rom(0x0000, 0x0A);
    let enabled_b = cartridge.describe_external_access(0xB000);
    assert_eq!(
        enabled_b.target(),
        CartridgeExternalTarget::Mbc6Ram {
            window: Mbc6Window::B,
            bank: 1,
        }
    );
    assert_eq!(
        enabled_b.availability(),
        CartridgeExternalAvailability::Accessible
    );
    cartridge.write_ram(0xA000, 0x11);
    cartridge.write_ram(0xB000, 0x22);
    assert_eq!(cartridge.read_ram(0xA000), 0x11);
    assert_eq!(cartridge.read_ram(0xB000), 0x22);

    cartridge.write_rom(0x0400, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x22);
    cartridge.write_ram(0xA000, 0x33);

    cartridge.write_rom(0x0800, 0x07);
    cartridge.write_ram(0xB000, 0x77);
    cartridge.write_rom(0x0400, 0x07);
    assert_eq!(cartridge.read_ram(0xA000), 0x77);

    cartridge.write_rom(0x0000, 0x00);
    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
}

#[test]
fn mbc6_flash_select_and_id_mode_are_cartridge_local() {
    let mut cartridge = loaded_mbc6();

    cartridge.write_rom(0x2800, 0x08);
    assert_eq!(cartridge.read_rom(0x4000), RAM_ABSENT_READ_VALUE);

    select_flash_window_a(&mut cartridge);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);

    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x90);
    assert_eq!(cartridge.read_rom(0x4000), 0xC2);
    assert_eq!(cartridge.read_rom(0x4001), 0x81);

    cartridge.write_rom(0x4000, 0xF0);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
}

#[test]
fn mbc6_window_b_flash_and_disabled_paths_are_explicit() {
    let mut cartridge = loaded_mbc6();

    assert_eq!(cartridge.read_rom(0x8000), RAM_ABSENT_READ_VALUE);
    cartridge.write_ram(0xA000, 0x44);
    cartridge.write_rom(0x4000, 0xAA);
    cartridge.write_rom(0x8000, 0x55);

    cartridge.write_rom(0x2800, 0x00);
    assert_eq!(cartridge.window_select_a, Mbc6WindowSelect::Rom);
    cartridge.write_rom(0x3800, 0x08);
    assert_eq!(cartridge.window_select_b, Mbc6WindowSelect::Flash);
    assert_eq!(cartridge.read_rom(0x6000), RAM_ABSENT_READ_VALUE);
    cartridge.write_rom(0x3800, 0x00);
    assert_eq!(cartridge.window_select_b, Mbc6WindowSelect::Rom);

    select_flash_window_b(&mut cartridge);
    cartridge.write_rom(0x6000, 0x12);

    flash_unlock_b(&mut cartridge);
    cartridge.write_rom(0x7555, 0x90);
    assert_eq!(cartridge.read_rom(0x6000), 0xC2);
    assert_eq!(cartridge.read_rom(0x6001), 0x81);
    assert_eq!(cartridge.read_rom(0x6002), RAM_ABSENT_READ_VALUE);

    cartridge.write_rom(0x0C00, 0x00);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);
    assert_eq!(cartridge.read_rom(0x6000), RAM_ABSENT_READ_VALUE);
}

#[test]
fn mbc6_programs_and_erases_main_flash_in_128_byte_blocks() {
    let mut cartridge = loaded_mbc6();
    select_flash_window_a(&mut cartridge);

    flash_program_command(&mut cartridge);
    commit_program_block_with_first_byte(&mut cartridge, 0x10, 0x0F, 0xAA);
    assert_eq!(cartridge.read_rom(0x4000), 0x0F);
    assert_eq!(cartridge.read_rom(0x407F), 0xAA);

    flash_program_command(&mut cartridge);
    commit_program_block_with_first_byte(&mut cartridge, 0x10, 0x33, 0x55);
    assert_eq!(cartridge.read_rom(0x4000), 0x03);
    assert_eq!(cartridge.read_rom(0x407F), 0x00);

    flash_erase_command(&mut cartridge);
    cartridge.write_rom(0x2000, 0x10);
    cartridge.write_rom(0x4000, 0x30);
    assert_eq!(cartridge.read_rom(0x4000), 0x80);
    cartridge.write_rom(0x4000, 0xF0);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
    assert_eq!(cartridge.read_rom(0x407F), 0xFF);
}

#[test]
fn mbc6_flash_command_fallbacks_return_to_read_array() {
    let mut cartridge = loaded_mbc6();
    select_flash_window_a(&mut cartridge);

    cartridge.write_rom(0x2000, 0x02);
    cartridge.write_rom(0x5555, 0xAA);
    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x12);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x80);
    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x80);
    cartridge.write_rom(0x5555, 0xAA);
    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_erase_command(&mut cartridge);
    cartridge.write_rom(0x5555, 0x99);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x60);
    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x60);
    cartridge.write_rom(0x5555, 0xAA);
    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_extended_command(&mut cartridge, 0x04);
    assert_eq!(cartridge.read_rom(0x4000), 0x80);
    reset_flash(&mut cartridge);

    flash_extended_command(&mut cartridge, 0x20);
    assert_eq!(cartridge.read_rom(0x4000), 0x80);
    reset_flash(&mut cartridge);

    flash_extended_command(&mut cartridge, 0x99);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x77);
    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x77);
    cartridge.write_rom(0x5555, 0xAA);
    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);

    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x77);
    flash_unlock(&mut cartridge);
    cartridge.write_rom(0x5555, 0x12);
    assert_eq!(cartridge.flash_mode, Mbc6FlashMode::ReadArray);
}

#[test]
fn mbc6_chip_erase_respects_sector0_write_protect() {
    let mut cartridge = loaded_mbc6();
    select_flash_window_a(&mut cartridge);

    cartridge.write_rom(0x1000, 0x01);
    flash_program_command(&mut cartridge);
    commit_program_block_with_first_byte(&mut cartridge, 0x00, 0x12, 0x34);
    assert_eq!(cartridge.read_rom(0x4000), 0x12);

    cartridge.write_rom(0x1000, 0x00);
    flash_program_command(&mut cartridge);
    commit_program_block_with_first_byte(&mut cartridge, 0x10, 0x56, 0x78);
    cartridge.write_rom(0x2000, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0x56);

    flash_erase_command(&mut cartridge);
    cartridge.write_rom(0x5555, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0x80);
    reset_flash(&mut cartridge);

    cartridge.write_rom(0x2000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x12);
    cartridge.write_rom(0x2000, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);

    cartridge.write_rom(0x1000, 0x01);
    flash_extended_command(&mut cartridge, 0x20);
    assert_eq!(cartridge.read_rom(0x4000), 0x82);
    reset_flash(&mut cartridge);

    flash_program_command(&mut cartridge);
    cartridge.write_rom(0x2000, 0x10);
    cartridge.write_rom(0x4000, 0x66);
    cartridge.write_rom(0x407F, 0x88);
    cartridge.write_rom(0x407F, 0x88);
    assert_eq!(cartridge.read_rom(0x4000), 0x82);
    reset_flash(&mut cartridge);
    cartridge.write_rom(0x2000, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0x66);

    flash_erase_command(&mut cartridge);
    cartridge.write_rom(0x5555, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0x82);
    reset_flash(&mut cartridge);

    cartridge.write_rom(0x2000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x12);
    cartridge.write_rom(0x2000, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
}

#[test]
fn mbc6_hidden_program_commit_uses_live_write_enable() {
    let mut cartridge = loaded_mbc6();
    select_flash_window_a(&mut cartridge);

    cartridge.write_rom(0x1000, 0x01);
    flash_extended_command(&mut cartridge, 0xE0);
    cartridge.write_rom(0x4000, 0x66);
    cartridge.write_rom(0x1000, 0x00);
    cartridge.write_rom(0x407F, 0x77);
    cartridge.write_rom(0x407F, 0x77);
    assert_eq!(cartridge.read_rom(0x4000), 0x80);
    reset_flash(&mut cartridge);

    flash_hidden_read_command(&mut cartridge);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
    reset_flash(&mut cartridge);
}

#[test]
fn mbc6_defensive_empty_backing_paths_do_not_mutate_or_panic() {
    let mut cartridge = loaded_mbc6();

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.ram.clear();
    cartridge.write_ram(0xA000, 0x99);
    assert!(cartridge.ram.is_empty());

    select_flash_window_a(&mut cartridge);
    cartridge.flash_mode = Mbc6FlashMode::Program(Mbc6ProgramState {
        target: Mbc6ProgramTarget::MainFlash,
        block_base: Some(2 * MBC6_ROM_FLASH_BANK_BYTES),
        buffer: vec![],
        written: vec![],
        final_byte_seen: false,
    });
    cartridge.write_rom(0x4000, 0x55);

    let Mbc6FlashMode::Program(state) = &cartridge.flash_mode else {
        panic!("program command with empty defensive buffers should remain pending");
    };
    assert!(state.buffer.is_empty());
    assert!(state.written.is_empty());
}

#[test]
fn mbc6_flash_write_enable_only_controls_sector0_hidden_and_protection_commands() {
    let mut cartridge = loaded_mbc6();
    select_flash_window_a(&mut cartridge);

    flash_program_command(&mut cartridge);
    commit_program_block_with_first_byte(&mut cartridge, 0x00, 0x12, 0x34);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);

    cartridge.write_rom(0x1000, 0x01);
    flash_program_command(&mut cartridge);
    commit_program_block_with_first_byte(&mut cartridge, 0x00, 0x12, 0x34);
    assert_eq!(cartridge.read_rom(0x4000), 0x12);

    flash_extended_command(&mut cartridge, 0x20);
    assert_eq!(cartridge.read_rom(0x4000), 0x82);
    cartridge.write_rom(0x4000, 0xF0);

    flash_erase_command(&mut cartridge);
    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x4000, 0x30);
    assert_eq!(cartridge.read_rom(0x4000), 0x82);
    cartridge.write_rom(0x4000, 0xF0);
    assert_eq!(cartridge.read_rom(0x4000), 0x12);

    flash_extended_command(&mut cartridge, 0x40);
    assert_eq!(cartridge.read_rom(0x4000), 0x80);
    cartridge.write_rom(0x4000, 0xF0);
    flash_erase_command(&mut cartridge);
    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x4000, 0x30);
    cartridge.write_rom(0x4000, 0xF0);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
}

#[test]
fn mbc6_hidden_region_uses_the_extended_flash_commands_and_hidden_read_mode() {
    let mut cartridge = loaded_mbc6();
    select_flash_window_a(&mut cartridge);

    flash_extended_command(&mut cartridge, 0xE0);
    assert_eq!(cartridge.read_rom(0x4000), 0x80);
    cartridge.write_rom(0x4000, 0xF0);

    cartridge.write_rom(0x1000, 0x01);
    flash_extended_command(&mut cartridge, 0xE0);
    commit_program_block_with_first_byte(&mut cartridge, 0x02, 0x5A, 0xA5);

    flash_hidden_read_command(&mut cartridge);
    assert_eq!(cartridge.read_rom(0x4000), 0x5A);
    assert_eq!(cartridge.read_rom(0x407F), 0xA5);

    cartridge.write_rom(0x4000, 0xF0);
    flash_extended_command(&mut cartridge, 0x04);
    cartridge.write_rom(0x4000, 0xF0);
    flash_hidden_read_command(&mut cartridge);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
    assert_eq!(cartridge.read_rom(0x407F), 0xFF);
}

#[test]
fn mbc6_persists_sram_main_flash_hidden_flash_and_sector0_protection() {
    let mut cartridge = loaded_mbc6();
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x11);

    let mut flash = vec![0xFF; MBC6_FLASH_BYTES];
    flash[0x20000] = 0x44;
    let mut hidden_region = vec![0xFF; MBC6_HIDDEN_BYTES];
    hidden_region[0] = 0x55;
    let restored = PersistentCartState::Mbc6 {
        ram: vec![0x22; MBC6_SUPPORTED_RAM_BYTES],
        flash,
        hidden_region,
        sector0_protected: true,
    };
    cartridge
        .restore_persistent_state(&restored)
        .expect("MBC6 state should restore");
    assert_eq!(cartridge.persistent_state(), restored);
    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::Mbc6 {
            ram: vec![0; 4],
            flash: vec![0xFF; MBC6_FLASH_BYTES],
            hidden_region: vec![0xFF; MBC6_HIDDEN_BYTES],
            sector0_protected: false,
        }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: MBC6_SUPPORTED_RAM_BYTES,
            actual: 4,
        }),
    );
    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::Mbc5Ram { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc6",
            actual: "Mbc5Ram",
        }),
    );
}

#[test]
fn mbc6_persistent_state_rejects_flash_hidden_and_non_battery_shapes() {
    let mut cartridge = loaded_mbc6();

    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::Mbc6 {
            ram: vec![0; MBC6_SUPPORTED_RAM_BYTES],
            flash: vec![0xFF; MBC6_FLASH_BYTES - 1],
            hidden_region: vec![0xFF; MBC6_HIDDEN_BYTES],
            sector0_protected: false,
        }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: MBC6_FLASH_BYTES,
            actual: MBC6_FLASH_BYTES - 1,
        }),
    );
    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::Mbc6 {
            ram: vec![0; MBC6_SUPPORTED_RAM_BYTES],
            flash: vec![0xFF; MBC6_FLASH_BYTES],
            hidden_region: vec![0xFF; MBC6_HIDDEN_BYTES - 1],
            sector0_protected: false,
        }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: MBC6_HIDDEN_BYTES,
            actual: MBC6_HIDDEN_BYTES - 1,
        }),
    );

    cartridge.has_battery = false;
    assert_eq!(cartridge.persistent_state(), PersistentCartState::None);
    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::None),
        Ok(()),
    );
    assert_eq!(
        cartridge.restore_persistent_state(&PersistentCartState::Mbc6 {
            ram: vec![0; MBC6_SUPPORTED_RAM_BYTES],
            flash: vec![0xFF; MBC6_FLASH_BYTES],
            hidden_region: vec![0xFF; MBC6_HIDDEN_BYTES],
            sector0_protected: false,
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc6",
        }),
    );
}

#[test]
fn mbc6_runtime_save_state_validates_all_nonvolatile_shapes() {
    let report = CartridgeSlot::load(build_banked_mbc6_rom(), &CompatibilityPolicy::strict())
        .expect("MBC6 should load");
    let cartridge = report.cartridge();
    let mut state = cartridge.capture_save_state();

    let Some(CartridgeDeviceSaveState::Mbc6(saved)) = &mut state.device else {
        panic!("expected MBC6 save state");
    };
    saved.flash.pop();

    assert!(matches!(
        cartridge.validate_save_state(&state),
        Err(CartridgeRuntimeSaveStateError::RamShapeMismatch {
            field: "MBC6 flash",
            expected,
            actual,
        }) if expected == Some(MBC6_FLASH_BYTES) && actual == Some(MBC6_FLASH_BYTES - 1)
    ));

    let mut state = cartridge.capture_save_state();
    let Some(CartridgeDeviceSaveState::Mbc6(saved)) = &mut state.device else {
        panic!("expected MBC6 save state");
    };
    saved.hidden_region.pop();

    assert!(matches!(
        cartridge.validate_save_state(&state),
        Err(CartridgeRuntimeSaveStateError::RamShapeMismatch {
            field: "MBC6 hidden flash",
            expected,
            actual,
        }) if expected == Some(MBC6_HIDDEN_BYTES) && actual == Some(MBC6_HIDDEN_BYTES - 1)
    ));
}

#[test]
fn mbc6_runtime_save_state_restores_registers_and_program_buffer_payload() {
    let (source, _) = CartridgeSlot::load(build_banked_mbc6_rom(), &CompatibilityPolicy::strict())
        .expect("MBC6 should load")
        .into_parts();
    let mut state = source.capture_save_state();
    let Some(CartridgeDeviceSaveState::Mbc6(saved)) = &mut state.device else {
        panic!("expected MBC6 save state");
    };
    let flash_mode = Mbc6FlashMode::Program(Mbc6ProgramState {
        target: Mbc6ProgramTarget::MainFlash,
        block_base: Some(MBC6_FLASH_PROGRAM_BLOCK_BYTES),
        buffer: vec![0xA5; MBC6_FLASH_PROGRAM_BLOCK_BYTES],
        written: vec![true; MBC6_FLASH_PROGRAM_BLOCK_BYTES],
        final_byte_seen: true,
    });

    saved.ram[0] = 0x12;
    saved.flash[0] = 0x34;
    saved.hidden_region[0] = 0x56;
    saved.ram_enabled = true;
    saved.flash_enabled = true;
    saved.flash_write_enabled = true;
    saved.ram_bank_a = 7;
    saved.ram_bank_b = 6;
    saved.rom_flash_bank_a = 5;
    saved.rom_flash_bank_b = 4;
    saved.window_select_a = Mbc6WindowSelect::Flash;
    saved.window_select_b = Mbc6WindowSelect::Rom;
    saved.sector0_protected = true;
    saved.flash_mode = flash_mode.clone();

    assert_eq!(
        state.dynamic_payload_bytes(),
        MBC6_SUPPORTED_RAM_BYTES
            + MBC6_FLASH_BYTES
            + MBC6_HIDDEN_BYTES
            + MBC6_FLASH_PROGRAM_BLOCK_BYTES
            + MBC6_FLASH_PROGRAM_BLOCK_BYTES * std::mem::size_of::<bool>(),
    );

    let (mut restored, _) =
        CartridgeSlot::load(build_banked_mbc6_rom(), &CompatibilityPolicy::strict())
            .expect("MBC6 should load")
            .into_parts();
    restored
        .validate_save_state(&state)
        .expect("mutated MBC6 save state should keep valid nonvolatile shapes");
    restored.restore_save_state(&state);

    let Some(CartridgeDevice::Mbc6(cartridge)) = restored.device.as_ref() else {
        panic!("expected restored MBC6 cartridge");
    };
    assert_eq!(cartridge.ram[0], 0x12);
    assert_eq!(cartridge.flash[0], 0x34);
    assert_eq!(cartridge.hidden_region[0], 0x56);
    assert!(cartridge.ram_enabled);
    assert!(cartridge.flash_enabled);
    assert!(cartridge.flash_write_enabled);
    assert_eq!(cartridge.ram_bank_a, 7);
    assert_eq!(cartridge.ram_bank_b, 6);
    assert_eq!(cartridge.rom_flash_bank_a, 5);
    assert_eq!(cartridge.rom_flash_bank_b, 4);
    assert_eq!(cartridge.window_select_a, Mbc6WindowSelect::Flash);
    assert_eq!(cartridge.window_select_b, Mbc6WindowSelect::Rom);
    assert!(cartridge.sector0_protected);
    assert_eq!(cartridge.flash_mode, flash_mode);
    assert_eq!(restored.capture_save_state(), state);
}
