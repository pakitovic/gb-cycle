use super::*;

#[test]
fn external_save_round_trips_mbc6_sram_plus_main_flash_when_hidden_state_is_default() {
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRamAndFlash {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
            flash_byte_len: 4,
            hidden_byte_len: 3,
        },
    };
    let state = PersistentCartState::Mbc6 {
        ram: vec![0x10, 0x20],
        flash: vec![0xFF, 0x7F, 0x3F, 0x1F],
        hidden_region: vec![0xFF; 3],
        sector0_protected: false,
    };

    let external = encode_external_cartridge_save(
        metadata,
        &state,
        1_700_000_000,
        ExternalSaveExportFormat::default(),
    )
    .expect("default MBC6 hidden state should export");
    assert_eq!(external, [0x10, 0x20, 0xFF, 0x7F, 0x3F, 0x1F]);

    let imported = import_external_cartridge_save(metadata, &state, &external, 1_700_000_001)
        .expect("default MBC6 hidden state should import");
    assert_eq!(imported, state);
}
