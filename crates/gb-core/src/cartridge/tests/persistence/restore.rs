use super::*;

#[test]
fn restore_persistent_state_validates_mbc2_nibble_payload_values() {
    let report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x06, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2+BATTERY should load");
    let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };

    let mut invalid_nibbles = [0u8; MBC2_RAM_CELL_COUNT];
    invalid_nibbles[7] = 0xF1;
    let error = cartridge
        .restore_persistent_state(&PersistentCartState::Mbc2Ram {
            ram_nibbles: invalid_nibbles,
        })
        .expect_err("invalid high bits must fail");

    assert_eq!(
        error,
        CartridgePersistentStateError::InvalidMbc2NibbleValue {
            index: 7,
            value: 0xF1,
        }
    );
}
