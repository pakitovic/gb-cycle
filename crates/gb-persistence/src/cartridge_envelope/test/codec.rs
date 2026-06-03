use super::*;

#[test]
fn encode_and_decode_round_trip_the_versioned_envelope() {
    let envelope = CartridgeSaveEnvelope {
        backend_metadata: CartridgeSaveBackendMetadata {
            format_version: CURRENT_SAVE_FORMAT_VERSION,
            saved_at_unix_seconds: 1_700_000_000,
        },
        cartridge_metadata: CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len: 4 },
            },
        },
        persistent_state: PersistentCartState::Mbc3RamRtc {
            ram: vec![0x11, 0x22, 0x33, 0x44],
            rtc: Mbc3RtcPersistentState {
                seconds: 59,
                minutes: 58,
                hours: 12,
                day_counter: 0x101,
                halt: true,
                carry: false,
            },
        },
    };

    assert_round_trip(envelope);
}

#[test]
fn round_trip_covers_remaining_profile_and_state_variants() {
    let mut huc3_mcu_ram = [0; 256];
    huc3_mcu_ram[0] = 0x0A;
    huc3_mcu_ram[1] = 0x0B;
    huc3_mcu_ram[255] = 0x0F;

    let profiles_and_states = vec![
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 11,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: false,
                has_rtc: false,
                profile: CartridgePersistenceProfile::None,
            },
            persistent_state: PersistentCartState::None,
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 12,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: false,
                has_rtc: false,
                profile: CartridgePersistenceProfile::NonPersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
                },
            },
            persistent_state: PersistentCartState::NoMbcRam {
                ram: vec![0x11, 0x22],
            },
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 13,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: true,
                profile: CartridgePersistenceProfile::PersistentRtc,
            },
            persistent_state: PersistentCartState::Mbc3Rtc {
                rtc: Mbc3RtcPersistentState {
                    seconds: 59,
                    minutes: 58,
                    hours: 7,
                    day_counter: 0x81,
                    halt: false,
                    carry: true,
                },
            },
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 14,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 3 },
                },
            },
            persistent_state: PersistentCartState::Mbc3Ram {
                ram: vec![0x33, 0x44, 0x55],
            },
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 15,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
                },
            },
            persistent_state: PersistentCartState::Mbc5Ram {
                ram: vec![0x66, 0x77],
            },
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 151,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRamAndFlash {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 2 },
                    flash_byte_len: 4,
                    hidden_byte_len: 3,
                },
            },
            persistent_state: PersistentCartState::Mbc6 {
                ram: vec![0x12, 0x34],
                flash: vec![0xFF, 0xFE, 0xFC, 0xF8],
                hidden_region: vec![0xAA, 0xBB, 0xCC],
                sector0_protected: true,
            },
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 16,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 4 },
                },
            },
            persistent_state: PersistentCartState::Mmm01Ram {
                ram: vec![0x88, 0x99, 0xAA, 0xBB],
            },
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 17,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 3 },
                },
            },
            persistent_state: PersistentCartState::Huc1Ram {
                ram: vec![0xCC, 0xDD, 0xEE],
            },
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 18,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: true,
                profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 5 },
                },
            },
            persistent_state: PersistentCartState::Huc3 {
                ram: vec![0x01, 0x23, 0x45, 0x67, 0x89],
                mcu_ram: huc3_mcu_ram,
                rtc: Huc3RtcPersistentState {
                    current_minutes_of_day: 1439,
                    current_days: 0x0FFF,
                    current_subminute_seconds: 59,
                    event_minutes_of_day: 123,
                    event_days: 0x0123,
                },
                rom_bank: 0x3F,
                ram_bank: 0x02,
                select_mode: 0x0E,
                access_address: 0xA5,
                mailbox_command: 0x06,
                mailbox_argument: 0x02,
                last_response_nybble: 0x01,
                semaphore_ready: true,
                ir_emitter_on: true,
                ir_light_detected: false,
                last_control_write: Some(0x77),
                last_unsupported_command: Some(0x06),
                last_unsupported_argument: Some(0x0E),
            },
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 19,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear { byte_len: 4 },
                },
            },
            persistent_state: PersistentCartState::PocketCameraRam {
                ram: vec![0x88, 0x99, 0xAA, 0xBB],
            },
        },
        CartridgeSaveEnvelope {
            backend_metadata: CartridgeSaveBackendMetadata {
                format_version: CURRENT_SAVE_FORMAT_VERSION,
                saved_at_unix_seconds: 20,
            },
            cartridge_metadata: CartridgePersistenceMetadata {
                has_battery: false,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentEeprom { byte_len: 4 },
            },
            persistent_state: PersistentCartState::Mbc7Eeprom {
                eeprom: vec![0x12, 0x34, 0xAB, 0xCD],
            },
        },
    ];

    for envelope in profiles_and_states {
        assert_round_trip(envelope);
    }

    let mut rtc_only_state = PersistentCartState::Mbc3Rtc {
        rtc: Mbc3RtcPersistentState {
            seconds: 59,
            minutes: 59,
            hours: 23,
            day_counter: 511,
            halt: false,
            carry: false,
        },
    };
    apply_elapsed_off_session_seconds(&mut rtc_only_state, 2);
    assert_eq!(
        rtc_only_state,
        PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 1,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: true,
            },
        }
    );

    let mut huc3_state = PersistentCartState::Huc3 {
        ram: vec![],
        mcu_ram: [0; 256],
        rtc: Huc3RtcPersistentState {
            current_minutes_of_day: 1439,
            current_days: 0x0FFF,
            current_subminute_seconds: 59,
            event_minutes_of_day: 3,
            event_days: 0,
        },
        rom_bank: 0,
        ram_bank: 0,
        select_mode: 0x0D,
        access_address: 0,
        mailbox_command: 0,
        mailbox_argument: 0,
        last_response_nybble: 0,
        semaphore_ready: true,
        ir_emitter_on: false,
        ir_light_detected: false,
        last_control_write: None,
        last_unsupported_command: None,
        last_unsupported_argument: None,
    };
    apply_elapsed_off_session_seconds(&mut huc3_state, 2);
    assert_eq!(
        huc3_state,
        PersistentCartState::Huc3 {
            ram: vec![],
            mcu_ram: [0; 256],
            rtc: Huc3RtcPersistentState {
                current_minutes_of_day: 0,
                current_days: 0,
                current_subminute_seconds: 1,
                event_minutes_of_day: 3,
                event_days: 0,
            },
            rom_bank: 0,
            ram_bank: 0,
            select_mode: 0x0D,
            access_address: 0,
            mailbox_command: 0,
            mailbox_argument: 0,
            last_response_nybble: 0,
            semaphore_ready: true,
            ir_emitter_on: false,
            ir_light_detected: false,
            last_control_write: None,
            last_unsupported_command: None,
            last_unsupported_argument: None,
        }
    );
}
