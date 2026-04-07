use super::*;

#[test]
fn no_mbc_loader_builds_rom_only_and_ram_variants() {
    let rom_only = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x00, 0x00, 0x00);
    let with_ram = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);

    let rom_only_report = CartridgeSlot::load(rom_only, &CompatibilityPolicy::strict())
        .expect("ROM-only NoMBC should load");
    let with_ram_report = CartridgeSlot::load(with_ram, &CompatibilityPolicy::strict())
        .expect("RAM NoMBC should load");

    assert_eq!(
        rom_only_report.cartridge().state(),
        CartridgeSlotState::NoMbc
    );
    assert_eq!(
        with_ram_report.cartridge().state(),
        CartridgeSlotState::NoMbc
    );
    assert_eq!(
        rom_only_report.cartridge().read_ram(0xA000),
        RAM_ABSENT_READ_VALUE
    );
    assert!(with_ram_report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rare but still treated as a valid No MBC variant")
    }));
}

#[test]
fn strict_validation_rejects_invalid_no_mbc_ram_configuration() {
    let rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x03);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("invalid RAM config must fail");

    match error {
        CartridgeLoadError::Rejected {
            classification,
            execution_mode,
            reason,
            ..
        } => {
            assert_eq!(classification.detected_name(), "ROM+RAM");
            assert_eq!(execution_mode, ExecutionMode::Strict);
            assert!(reason.contains("expects RAM size code"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn warn_validation_can_admit_unambiguous_no_mbc_size_mismatches_with_diagnostics() {
    let rom = build_test_rom(64 * 1024, 0x00, 0x01, 0x00);
    let report =
        CartridgeSlot::load(rom, &warn_policy()).expect("warn policy should admit the mismatch");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::NoMbc);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expects ROM size code 0x00"))
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expects a 32 KiB image"))
    );
}

#[test]
fn warn_validation_can_admit_unambiguous_no_mbc_ram_header_mismatches_with_diagnostics() {
    let rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x01);
    let report = CartridgeSlot::load(rom, &warn_policy())
        .expect("warn policy should admit the legacy RAM-size mismatch");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::NoMbc);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expects RAM size code 0x02"))
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unsupported RAM configuration"))
    );
}
