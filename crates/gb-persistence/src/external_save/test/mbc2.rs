use super::*;

#[test]
fn external_save_exports_mbc2_in_packed_form_and_imports_one_byte_per_nibble_form() {
    let metadata = CartridgePersistenceMetadata {
        has_battery: true,
        has_rtc: false,
        profile: CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                cell_count: MBC2_RAM_NIBBLE_COUNT,
            },
        },
    };
    let mut ram_nibbles = [0; MBC2_RAM_NIBBLE_COUNT];
    ram_nibbles[0] = 0x01;
    ram_nibbles[1] = 0x02;
    ram_nibbles[2] = 0x0A;
    ram_nibbles[3] = 0x0B;
    ram_nibbles[511] = 0x0F;
    let state = PersistentCartState::Mbc2Ram { ram_nibbles };

    let external = encode_external_cartridge_save(
        metadata,
        &state,
        1_700_000_000,
        ExternalSaveExportFormat::default(),
    )
    .expect("MBC2 should export");
    assert_eq!(external.len(), MBC2_MGBA_PACKED_BYTE_COUNT);
    assert_eq!(external[0], 0x21);
    assert_eq!(external[1], 0xBA);
    assert_eq!(external[255], 0xF0);
    assert_eq!(
        import_external_cartridge_save(metadata, &state, &external, 1_700_000_000)
            .expect("mGBA packed MBC2 should import"),
        state
    );

    let mut one_byte_per_nibble = vec![0; MBC2_RAM_NIBBLE_COUNT];
    one_byte_per_nibble[0] = 0xF1;
    one_byte_per_nibble[1] = 0xE2;
    one_byte_per_nibble[2] = 0xCA;
    one_byte_per_nibble[3] = 0xBB;
    one_byte_per_nibble[511] = 0xFF;
    let imported =
        import_external_cartridge_save(metadata, &state, &one_byte_per_nibble, 1_700_000_000)
            .expect("one-byte-per-nibble MBC2 should import");
    let PersistentCartState::Mbc2Ram { ram_nibbles } = imported else {
        panic!("expected MBC2 state");
    };
    assert_eq!(ram_nibbles[0], 0x01);
    assert_eq!(ram_nibbles[1], 0x02);
    assert_eq!(ram_nibbles[2], 0x0A);
    assert_eq!(ram_nibbles[3], 0x0B);
    assert_eq!(ram_nibbles[511], 0x0F);
}
