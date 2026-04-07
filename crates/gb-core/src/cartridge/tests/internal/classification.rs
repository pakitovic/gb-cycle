use super::*;

#[test]
fn classification_and_private_helper_paths_cover_remaining_documented_types_and_flags() {
    let mmm01_ram = CartridgeClassification::classify(0x0C);
    assert_eq!(mmm01_ram.detected_name(), "MMM01+RAM");
    assert_eq!(
        mmm01_ram.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::DocumentedButUnsupported)
    );

    let mmm01_battery = CartridgeClassification::classify(0x0D);
    assert_eq!(mmm01_battery.detected_name(), "MMM01+RAM+BATTERY");
    assert_eq!(
        mmm01_battery.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::DocumentedButUnsupported)
    );

    let tama5 = CartridgeClassification::classify(0xFD);
    assert_eq!(tama5.detected_name(), "BANDAI TAMA5");
    assert_eq!(
        tama5.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::AccessorySpecialCase)
    );

    assert_eq!(decode_cgb_flag(0xC0), CgbFlag::Only);
    assert_eq!(decode_cgb_flag(0xA0), CgbFlag::SupportedNonCanonical(0xA0));
    assert_eq!(decode_cgb_flag(0x42), CgbFlag::Unknown(0x42));
    assert_eq!(decode_sgb_flag(0x7F), SgbFlag::Unknown(0x7F));
    assert_eq!(expected_ram_code_decompressed(0x99), 0);
    assert!(!matches_padded_title(b"GB", b"GBTEST"));

    let rtc = Mbc3RtcPersistentState {
        seconds: 1,
        minutes: 2,
        hours: 3,
        day_counter: 0x0104,
        halt: true,
        carry: true,
    };
    assert_eq!(
        Mbc3RtcState::from(rtc),
        Mbc3RtcState {
            seconds: 1,
            minutes: 2,
            hours: 3,
            day_counter: 0x0104,
            halt: true,
            carry: true,
        }
    );
    assert_eq!(PersistentCartState::Mbc3Rtc { rtc }.kind_name(), "Mbc3Rtc");
    assert_eq!(
        PersistentCartState::Mbc3RamRtc { ram: vec![], rtc }.kind_name(),
        "Mbc3RamRtc"
    );
    assert_eq!(
        PersistentCartState::Mbc5Ram { ram: vec![] }.kind_name(),
        "Mbc5Ram"
    );

    let mut diagnostics = Vec::new();
    assert_eq!(
        record_degradable_issue(
            &mut diagnostics,
            ValidationPolicy::Ignore,
            "ignored warning".to_owned(),
        ),
        Ok(())
    );
    assert!(diagnostics.is_empty());
}
