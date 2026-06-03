use super::*;

#[test]
fn manual_sessions_expose_their_policy_and_do_not_flush_on_close_without_changes() {
    let root = temp_save_root();
    let mut machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    let key = CartridgeSaveKey::new("manual".to_string()).expect("key should be valid");
    let mut session = DesktopSaveSession::open(
        Some(&root),
        DesktopSaveFlushPolicy::Manual,
        Some(key.clone()),
        &mut machine,
    )
    .expect("manual save session should open")
    .expect("battery-backed cartridge should create a session");

    assert_eq!(session.flush_policy(), DesktopSaveFlushPolicy::Manual);
    assert_eq!(
        session.save_path(),
        root.join(format!("{}.sav", key.as_str()))
    );
    assert!(
        !session
            .flush_if_changed(&machine, "no-op")
            .expect("unchanged state should be a no-op")
    );
    session
        .close(&machine)
        .expect("manual close without changes should not fail");
    assert!(!session.save_path().exists());

    fs::remove_dir_all(root).expect("temp save root should be removable");
}

#[test]
fn persistence_helpers_cover_rtc_advancement_and_error_formatting() {
    let mut rtc_state = PersistentCartState::Mbc3Rtc {
        rtc: gb_core::Mbc3RtcPersistentState {
            seconds: 58,
            minutes: 59,
            hours: 23,
            day_counter: 0,
            halt: false,
            carry: false,
        },
    };
    apply_elapsed_off_session_seconds(&mut rtc_state, 2);
    assert!(matches!(rtc_state, PersistentCartState::Mbc3Rtc { .. }));
    if let PersistentCartState::Mbc3Rtc { rtc } = rtc_state {
        assert_eq!(rtc.seconds, 0);
        assert_eq!(rtc.minutes, 0);
        assert_eq!(rtc.hours, 0);
        assert_eq!(rtc.day_counter, 1);
    }

    let mut huc3_state = PersistentCartState::Huc3 {
        ram: vec![0x11; 8],
        mcu_ram: [0; 256],
        rtc: gb_core::Huc3RtcPersistentState {
            current_minutes_of_day: 1,
            current_days: 0,
            current_subminute_seconds: 59,
            event_minutes_of_day: 5,
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
    if let PersistentCartState::Huc3 { rtc, .. } = huc3_state {
        assert_eq!(rtc.current_minutes_of_day, 2);
        assert_eq!(rtc.current_subminute_seconds, 1);
    } else {
        panic!("expected Huc3 state");
    }

    let mut plain_ram = PersistentCartState::Mbc5Ram { ram: vec![1, 2, 3] };
    let before = plain_ram.clone();
    apply_elapsed_off_session_seconds(&mut plain_ram, 120);
    assert_eq!(plain_ram, before);

    assert!(
        format_restore_error(CartridgePersistentStateError::KindMismatch {
            expected: "MBC2",
            actual: "MBC3",
        })
        .contains("KindMismatch")
    );
    assert!(
        format_load_error(CartridgeLoadError::HeaderParse(
            gb_core::CartridgeHeaderParseError::ImageTooSmall {
                actual_size: 4,
                minimum_size: 0x150,
            },
        ))
        .contains("HeaderParse")
    );

    let machine = load_machine(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    _cartridge(machine.cartridge());
}
