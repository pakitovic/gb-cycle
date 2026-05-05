use super::*;
use crate::scheduler::TCycle;

fn load_mbc7() -> CartridgeSlot {
    CartridgeSlot::load(build_banked_mbc7_rom(0x03), &CompatibilityPolicy::strict())
        .expect("MBC7 should load")
        .cartridge
}

fn enable_mbc7_registers(cartridge: &mut CartridgeSlot) {
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x40);
}

fn eeprom_write_pins(cartridge: &mut CartridgeSlot, cs: bool, clk: bool, di: bool) {
    let value = (u8::from(cs) << 7) | (u8::from(clk) << 6) | (u8::from(di) << 1);
    cartridge.write_ram(0xA080, value);
}

fn eeprom_clock_bit(cartridge: &mut CartridgeSlot, bit: bool) {
    eeprom_write_pins(cartridge, true, false, bit);
    eeprom_write_pins(cartridge, true, true, bit);
}

fn eeprom_send_command(cartridge: &mut CartridgeSlot, command: u16) {
    eeprom_write_pins(cartridge, false, false, false);
    eeprom_write_pins(cartridge, true, false, false);
    eeprom_clock_bit(cartridge, true);
    for bit in (0..10).rev() {
        eeprom_clock_bit(cartridge, (command >> bit) & 1 != 0);
    }
}

fn eeprom_send_data(cartridge: &mut CartridgeSlot, value: u16) {
    for bit in (0..16).rev() {
        eeprom_clock_bit(cartridge, (value >> bit) & 1 != 0);
    }
    eeprom_write_pins(cartridge, false, false, false);
}

fn eeprom_finish(cartridge: &mut CartridgeSlot) {
    eeprom_write_pins(cartridge, false, false, false);
}

fn eeprom_read_word(cartridge: &mut CartridgeSlot, address: u8) -> u16 {
    eeprom_send_command(cartridge, 0x0200 | u16::from(address & 0x7F));
    let mut value = 0u16;
    for _ in 0..16 {
        eeprom_write_pins(cartridge, true, false, false);
        eeprom_write_pins(cartridge, true, true, false);
        value = (value << 1) | u16::from(cartridge.read_ram(0xA080) & 0x01);
    }
    eeprom_finish(cartridge);
    value
}

fn eeprom_write_word(cartridge: &mut CartridgeSlot, address: u8, value: u16) {
    eeprom_send_command(cartridge, 0x0100 | u16::from(address & 0x7F));
    eeprom_send_data(cartridge, value);
}

#[test]
fn mbc7_header_loads_as_dedicated_mapper_without_rumble() {
    let cartridge = load_mbc7();
    let classification = cartridge
        .classification()
        .expect("classification should exist");

    assert_eq!(cartridge.state(), CartridgeSlotState::Mbc7);
    assert_eq!(
        classification.detected_name(),
        "MBC7+SENSOR+RUMBLE+RAM+BATTERY"
    );
    assert_eq!(
        classification.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc7)
    );
    assert!(cartridge.has_mbc7_accelerometer());
    assert!(!cartridge.has_rumble());
    assert!(!cartridge.rumble_on());
    assert_eq!(
        cartridge.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentEeprom {
                byte_len: MBC7_EEPROM_BYTES,
            },
        }
    );
}

#[test]
fn mbc7_strict_loader_rejects_sram_header_and_oversized_roms_without_mbc5_fallback() {
    let mut bad_ram = build_banked_mbc7_rom(0x03);
    bad_ram[RAM_SIZE_ADDRESS] = 0x02;
    let error = CartridgeSlot::load(bad_ram, &CompatibilityPolicy::strict())
        .expect_err("MBC7 should reject decoded SRAM headers");
    assert!(matches!(
        error,
        CartridgeLoadError::Rejected { classification, .. }
            if classification.selection() == CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc7)
    ));

    let oversized = build_banked_mbc7_rom(0x07);
    let error = CartridgeSlot::load(oversized, &CompatibilityPolicy::strict())
        .expect_err("MBC7 should reject ROMs beyond the current documented baseline");
    assert!(matches!(
        error,
        CartridgeLoadError::Rejected { classification, .. }
            if classification.detected_name() == "MBC7+SENSOR+RUMBLE+RAM+BATTERY"
    ));
}

#[test]
fn mbc7_strict_loader_rejects_invalid_size_mismatch_and_dmg_only_headers() {
    let mut invalid_size_code = build_banked_mbc7_rom(0x03);
    invalid_size_code[ROM_SIZE_ADDRESS] = 0x52;
    let error = CartridgeSlot::load(invalid_size_code, &CompatibilityPolicy::strict())
        .expect_err("MBC7 should reject undecodable ROM size codes");
    assert!(matches!(
        error,
        CartridgeLoadError::Rejected { classification, .. }
            if classification.selection() == CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc7)
    ));

    let mut mismatched_size = build_banked_mbc7_rom(0x03);
    mismatched_size.pop();
    let error = CartridgeSlot::load(mismatched_size, &CompatibilityPolicy::strict())
        .expect_err("MBC7 should reject ROMs whose byte length disagrees with the header");
    assert!(matches!(
        error,
        CartridgeLoadError::Rejected { classification, .. }
            if classification.selection() == CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc7)
    ));

    let mut dmg_only = build_banked_mbc7_rom(0x03);
    dmg_only[CGB_FLAG_ADDRESS] = 0x00;
    let error = CartridgeSlot::load(dmg_only, &CompatibilityPolicy::strict())
        .expect_err("MBC7 should reject non-CGB headers");
    assert!(matches!(
        error,
        CartridgeLoadError::Rejected { classification, .. }
            if classification.selection() == CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc7)
    ));
}

#[test]
fn mbc7_rom_banking_uses_only_the_low_seven_bank_bits() {
    let mut cartridge = load_mbc7();

    assert_eq!(cartridge.read_rom(0x0000), 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);

    cartridge.write_rom(0x2000, 0x02);
    assert_eq!(cartridge.read_rom(0x4000), 0x02);

    cartridge.write_rom(0x2000, 0x81);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);

    cartridge.write_rom(0x2000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);

    cartridge.write_rom(0x6000, 0x7F);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);
}

#[test]
fn mbc7_registers_require_both_enable_gates_and_b000_bfff_reads_ff() {
    let mut cartridge = load_mbc7();

    assert_eq!(cartridge.read_ram(0xA060), RAM_ABSENT_READ_VALUE);
    cartridge.write_rom(0x0000, 0x0A);
    assert_eq!(cartridge.read_ram(0xA060), RAM_ABSENT_READ_VALUE);
    cartridge.write_rom(0x4000, 0x3F);
    assert_eq!(cartridge.read_ram(0xA060), RAM_ABSENT_READ_VALUE);

    cartridge.write_rom(0x4000, 0x40);
    assert_eq!(cartridge.read_ram(0xA060), 0x00);
    assert_eq!(cartridge.read_ram(0xA070), RAM_ABSENT_READ_VALUE);
    assert_eq!(cartridge.read_ram(0xA090), RAM_ABSENT_READ_VALUE);
    assert_eq!(cartridge.read_ram(0xB000), RAM_ABSENT_READ_VALUE);
    assert_eq!(cartridge.read_ram(0xBFFF), RAM_ABSENT_READ_VALUE);
}

#[test]
fn mbc7_timed_register_access_uses_the_same_enabled_register_file() {
    let mut cartridge = load_mbc7();

    assert_eq!(
        cartridge.read_ram_timed(0xA060, TCycle::ZERO),
        RAM_ABSENT_READ_VALUE
    );
    cartridge.write_ram_timed(0xA000, 0x55, TCycle::new(1));
    cartridge.write_ram_timed(0xC000, 0x55, TCycle::new(2));
    assert_eq!(cartridge.read_ram(0xA020), RAM_ABSENT_READ_VALUE);

    enable_mbc7_registers(&mut cartridge);
    cartridge.write_ram_timed(0xA000, 0x55, TCycle::new(3));
    assert_eq!(cartridge.read_ram_timed(0xA020, TCycle::new(4)), 0x00);
    assert_eq!(cartridge.read_ram_timed(0xA030, TCycle::new(5)), 0x80);
}

#[test]
fn mbc7_accelerometer_latch_sequence_captures_host_input_deterministically() {
    let mut cartridge = load_mbc7();
    enable_mbc7_registers(&mut cartridge);

    assert_eq!(
        Mbc7AccelerometerInput::default(),
        Mbc7AccelerometerInput::neutral()
    );
    cartridge
        .set_mbc7_accelerometer_input(Mbc7AccelerometerInput::from_raw(0x8123, 0x8456))
        .expect("MBC7 accelerometer should be present");
    assert_eq!(cartridge.read_ram(0xA020), 0x00);
    assert_eq!(cartridge.read_ram(0xA030), 0x80);

    cartridge.write_ram(0xA010, 0xAA);
    assert_eq!(cartridge.read_ram(0xA020), 0x00);
    assert_eq!(cartridge.read_ram(0xA030), 0x80);

    cartridge.write_ram(0xA000, 0x55);
    assert_eq!(cartridge.read_ram(0xA020), 0x00);
    assert_eq!(cartridge.read_ram(0xA030), 0x80);
    cartridge.write_ram(0xA010, 0xAA);
    assert_eq!(cartridge.read_ram(0xA020), 0x23);
    assert_eq!(cartridge.read_ram(0xA030), 0x81);
    assert_eq!(cartridge.read_ram(0xA040), 0x56);
    assert_eq!(cartridge.read_ram(0xA050), 0x84);

    let mut machine = crate::Machine::new(crate::MachineConfig::new(crate::ConsoleModel::GameBoy));
    machine
        .load_cartridge(build_banked_mbc7_rom(0x03))
        .expect("MBC7 should load into Machine");
    assert!(machine.has_mbc7_accelerometer());
    machine
        .set_mbc7_accelerometer_input(Mbc7AccelerometerInput::from_milli_g(1000, -1000))
        .expect("Machine should route deterministic MBC7 accelerometer input");

    let mut empty = CartridgeSlot::empty();
    assert!(!empty.has_mbc7_accelerometer());
    assert_eq!(
        empty.set_mbc7_accelerometer_input(Mbc7AccelerometerInput::neutral()),
        Err(Mbc7AccelerometerError::UnsupportedCartridge)
    );

    let mut no_mbc = CartridgeSlot::load(
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC should load")
    .cartridge;
    assert_eq!(
        no_mbc.set_mbc7_accelerometer_input(Mbc7AccelerometerInput::neutral()),
        Err(Mbc7AccelerometerError::UnsupportedCartridge)
    );

    let mut empty_machine =
        crate::Machine::new(crate::MachineConfig::new(crate::ConsoleModel::GameBoy));
    assert!(!empty_machine.has_mbc7_accelerometer());
    assert_eq!(
        empty_machine.set_mbc7_accelerometer_input(Mbc7AccelerometerInput::neutral()),
        Err(Mbc7AccelerometerError::UnsupportedCartridge)
    );
}

#[test]
fn mbc7_eeprom_commands_read_write_erase_and_persist_raw_256_bytes() {
    let mut cartridge = load_mbc7();
    enable_mbc7_registers(&mut cartridge);

    eeprom_write_pins(&mut cartridge, false, false, false);
    eeprom_write_pins(&mut cartridge, true, false, false);
    eeprom_clock_bit(&mut cartridge, false);
    eeprom_finish(&mut cartridge);

    assert_eq!(eeprom_read_word(&mut cartridge, 0x12), 0xFFFF);

    eeprom_send_command(&mut cartridge, 0x0301);
    eeprom_finish(&mut cartridge);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x01), 0xFFFF);

    eeprom_send_command(&mut cartridge, 0x00C0);
    eeprom_finish(&mut cartridge);
    eeprom_write_word(&mut cartridge, 0x12, 0xA55A);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x12), 0xA55A);

    let PersistentCartState::Mbc7Eeprom { eeprom } = cartridge.persistent_state() else {
        panic!("expected MBC7 EEPROM state");
    };
    assert_eq!(eeprom.len(), MBC7_EEPROM_BYTES);
    assert_eq!(&eeprom[0x24..0x26], &[0xA5, 0x5A]);

    eeprom_send_command(&mut cartridge, 0x0300 | 0x12);
    eeprom_finish(&mut cartridge);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x12), 0xFFFF);

    eeprom_send_command(&mut cartridge, 0x0040);
    eeprom_send_data(&mut cartridge, 0x1234);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x7F), 0x1234);

    eeprom_send_command(&mut cartridge, 0x0080);
    eeprom_finish(&mut cartridge);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x12), 0xFFFF);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x7F), 0xFFFF);

    eeprom_write_word(&mut cartridge, 0x7F, 0xBEEF);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x7F), 0xBEEF);
    eeprom_send_command(&mut cartridge, 0x0000);
    eeprom_finish(&mut cartridge);
    eeprom_send_command(&mut cartridge, 0x0300 | 0x7F);
    eeprom_finish(&mut cartridge);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x7F), 0xBEEF);
}

#[test]
fn mbc7_save_state_restores_eeprom_latch_and_serial_state() {
    let mut cartridge = load_mbc7();
    enable_mbc7_registers(&mut cartridge);
    cartridge.write_ram(0xA000, 0x55);
    eeprom_send_command(&mut cartridge, 0x00C0);
    eeprom_finish(&mut cartridge);
    eeprom_write_word(&mut cartridge, 0x01, 0xCAFE);

    let state = cartridge.capture_save_state();
    cartridge.write_ram(0xA000, 0x55);
    eeprom_write_word(&mut cartridge, 0x01, 0xBEEF);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x01), 0xBEEF);

    cartridge.restore_save_state(&state);
    enable_mbc7_registers(&mut cartridge);
    assert_eq!(eeprom_read_word(&mut cartridge, 0x01), 0xCAFE);
}

#[test]
fn mbc7_save_state_validation_rejects_eeprom_shape_mismatches_and_restores_serial_edges() {
    let mut cartridge = load_mbc7();
    enable_mbc7_registers(&mut cartridge);

    let mut wrong_shape = cartridge.capture_save_state();
    assert_eq!(wrong_shape.dynamic_payload_bytes(), MBC7_EEPROM_BYTES);
    let Some(CartridgeDeviceSaveState::Mbc7(saved)) = &mut wrong_shape.device else {
        panic!("expected MBC7 save-state");
    };
    saved.eeprom.pop();
    let error = cartridge
        .validate_save_state(&wrong_shape)
        .expect_err("MBC7 save-state should validate EEPROM shape");
    assert!(matches!(
        error,
        CartridgeRuntimeSaveStateError::RamShapeMismatch {
            field: "MBC7 EEPROM",
            expected: Some(MBC7_EEPROM_BYTES),
            actual: Some(actual),
        } if actual == MBC7_EEPROM_BYTES - 1
    ));

    let mut defensive_serial_state = cartridge.capture_save_state();
    let Some(CartridgeDeviceSaveState::Mbc7(saved)) = &mut defensive_serial_state.device else {
        panic!("expected MBC7 save-state");
    };
    saved.ram_enabled = true;
    saved.sensor_eeprom_enabled = true;
    saved.eeprom_pins = Mbc7EepromPins {
        cs: true,
        clk: false,
        di: false,
        do_pin: false,
    };
    saved.eeprom_command = Mbc7EepromCommand::SendingRead {
        bits_remaining: 0,
        value: 0x1234,
    };
    cartridge.restore_save_state(&defensive_serial_state);
    cartridge.write_ram(0xA080, 0xC0);
    assert_eq!(cartridge.read_ram(0xA080) & 0x01, 0x00);
}

#[test]
fn mbc7_persistent_restore_rejects_wrong_payload_shape_and_kind() {
    let mut cartridge = load_mbc7();

    let error = cartridge
        .restore_persistent_state(&PersistentCartState::Mbc7Eeprom {
            eeprom: vec![0xFF; MBC7_EEPROM_BYTES - 1],
        })
        .expect_err("MBC7 should reject short EEPROM payloads");
    assert!(matches!(
        error,
        CartridgePersistentStateError::EepromLengthMismatch {
            expected: MBC7_EEPROM_BYTES,
            actual,
        } if actual == MBC7_EEPROM_BYTES - 1
    ));

    let error = cartridge
        .restore_persistent_state(&PersistentCartState::Mbc5Ram { ram: vec![0x00] })
        .expect_err("MBC7 should reject non-EEPROM persistent payloads");
    assert!(matches!(
        error,
        CartridgePersistentStateError::KindMismatch {
            expected: "Mbc7Eeprom",
            actual: "Mbc5Ram",
        }
    ));
}

#[test]
fn mbc7_external_access_descriptors_name_accelerometer_and_eeprom_registers() {
    let mut cartridge = load_mbc7();

    let no_device = cartridge.describe_external_access(0x9FFF);
    assert_eq!(no_device.target(), CartridgeExternalTarget::NoDevice);
    assert_eq!(
        no_device.availability(),
        CartridgeExternalAvailability::Absent
    );

    let fixed_open = cartridge.describe_external_access(0xB000);
    assert_eq!(
        fixed_open.target(),
        CartridgeExternalTarget::Mbc7ReservedRegister { selector: 0x10 }
    );
    assert_eq!(
        fixed_open.availability(),
        CartridgeExternalAvailability::Reserved
    );

    let disabled = cartridge.describe_external_access(0xA020);
    assert_eq!(
        disabled.availability(),
        CartridgeExternalAvailability::Disabled
    );
    assert_eq!(
        disabled.target(),
        CartridgeExternalTarget::Mbc7AccelerometerAxis {
            axis: Mbc7AccelerometerAxis::X,
            byte: Mbc7AccelerometerByte::Low,
        }
    );
    assert_eq!(
        disabled.read_behavior(),
        CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE)
    );

    enable_mbc7_registers(&mut cartridge);
    let latch_reset = cartridge.describe_external_access(0xA000);
    assert_eq!(
        latch_reset.target(),
        CartridgeExternalTarget::Mbc7AccelerometerLatchReset
    );
    assert_eq!(
        latch_reset.write_behavior(),
        CartridgeExternalWriteBehavior::Mbc7AccelerometerLatch
    );

    let latch_commit = cartridge.describe_external_access(0xA010);
    assert_eq!(
        latch_commit.target(),
        CartridgeExternalTarget::Mbc7AccelerometerLatchCommit
    );
    assert_eq!(
        latch_commit.write_behavior(),
        CartridgeExternalWriteBehavior::Mbc7AccelerometerLatch
    );

    assert_eq!(
        cartridge.describe_external_access(0xA030).target(),
        CartridgeExternalTarget::Mbc7AccelerometerAxis {
            axis: Mbc7AccelerometerAxis::X,
            byte: Mbc7AccelerometerByte::High,
        }
    );
    assert_eq!(
        cartridge.describe_external_access(0xA040).target(),
        CartridgeExternalTarget::Mbc7AccelerometerAxis {
            axis: Mbc7AccelerometerAxis::Y,
            byte: Mbc7AccelerometerByte::Low,
        }
    );

    let fixed_zero = cartridge.describe_external_access(0xA060);
    assert_eq!(
        fixed_zero.target(),
        CartridgeExternalTarget::Mbc7FixedRegister { value: 0x00 }
    );
    assert_eq!(
        fixed_zero.read_behavior(),
        CartridgeExternalReadBehavior::FallbackValue(0x00)
    );

    let reserved = cartridge.describe_external_access(0xA070);
    assert_eq!(
        reserved.target(),
        CartridgeExternalTarget::Mbc7ReservedRegister { selector: 0x07 }
    );
    assert_eq!(
        reserved.availability(),
        CartridgeExternalAvailability::Reserved
    );

    let axis = cartridge.describe_external_access(0xA050);
    assert_eq!(
        axis.availability(),
        CartridgeExternalAvailability::Accessible
    );
    assert_eq!(
        axis.read_behavior(),
        CartridgeExternalReadBehavior::Mbc7Accelerometer
    );
    assert_eq!(
        axis.write_behavior(),
        CartridgeExternalWriteBehavior::Ignored
    );

    let eeprom = cartridge.describe_external_access(0xA080);
    assert_eq!(
        eeprom.availability(),
        CartridgeExternalAvailability::Accessible
    );
    assert_eq!(eeprom.target(), CartridgeExternalTarget::Mbc7EepromSerial);
    assert_eq!(
        eeprom.read_behavior(),
        CartridgeExternalReadBehavior::Mbc7EepromSerial
    );
    assert_eq!(
        eeprom.write_behavior(),
        CartridgeExternalWriteBehavior::Mbc7EepromSerial
    );
}
