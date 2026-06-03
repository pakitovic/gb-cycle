use super::*;

#[test]
fn persistent_state_kind_names_cover_all_public_variants() {
    let huc3_state = PersistentCartState::Huc3 {
        ram: vec![],
        mcu_ram: [0; 256],
        rtc: Huc3RtcPersistentState {
            current_minutes_of_day: 0,
            current_days: 0,
            current_subminute_seconds: 0,
            event_minutes_of_day: 0,
            event_days: 0,
        },
        rom_bank: 0,
        ram_bank: 0,
        select_mode: 0,
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
    let states = [
        (PersistentCartState::None, "None"),
        (PersistentCartState::NoMbcRam { ram: vec![] }, "NoMbcRam"),
        (PersistentCartState::Mmm01Ram { ram: vec![] }, "Mmm01Ram"),
        (PersistentCartState::Huc1Ram { ram: vec![] }, "Huc1Ram"),
        (huc3_state, "Huc3"),
        (PersistentCartState::Mbc1Ram { ram: vec![] }, "Mbc1Ram"),
        (
            PersistentCartState::Mbc2Ram {
                ram_nibbles: [0; MBC2_RAM_NIBBLE_COUNT],
            },
            "Mbc2Ram",
        ),
        (
            PersistentCartState::Mbc3Rtc {
                rtc: Mbc3RtcPersistentState {
                    seconds: 0,
                    minutes: 0,
                    hours: 0,
                    day_counter: 0,
                    halt: false,
                    carry: false,
                },
            },
            "Mbc3Rtc",
        ),
        (PersistentCartState::Mbc3Ram { ram: vec![] }, "Mbc3Ram"),
        (
            PersistentCartState::Mbc3RamRtc {
                ram: vec![],
                rtc: Mbc3RtcPersistentState {
                    seconds: 0,
                    minutes: 0,
                    hours: 0,
                    day_counter: 0,
                    halt: false,
                    carry: false,
                },
            },
            "Mbc3RamRtc",
        ),
        (PersistentCartState::Mbc5Ram { ram: vec![] }, "Mbc5Ram"),
        (
            PersistentCartState::Mbc6 {
                ram: vec![],
                flash: vec![],
                hidden_region: vec![],
                sector0_protected: false,
            },
            "Mbc6",
        ),
        (
            PersistentCartState::Mbc7Eeprom { eeprom: vec![] },
            "Mbc7Eeprom",
        ),
        (
            PersistentCartState::PocketCameraRam { ram: vec![] },
            "PocketCameraRam",
        ),
    ];

    for (state, expected_name) in states {
        assert_eq!(persistent_state_kind_name(&state), expected_name);
    }
}
