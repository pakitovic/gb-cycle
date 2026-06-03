use super::*;

#[test]
fn cli_value_parsers_cover_save_policy_direction_source_and_face_layout() {
    assert_eq!(
        parse_save_policy("manual"),
        Ok(DesktopSaveFlushPolicy::Manual)
    );
    assert_eq!(
        parse_save_policy("on-close"),
        Ok(DesktopSaveFlushPolicy::OnClose)
    );
    assert_eq!(
        parse_save_policy("on-write"),
        Ok(DesktopSaveFlushPolicy::OnWrite)
    );
    assert_eq!(
        parse_save_policy("debounced"),
        Ok(DesktopSaveFlushPolicy::Debounced)
    );
    assert!(parse_save_policy("later").is_err());

    assert_eq!(
        parse_gamepad_directional_source("dpad-only"),
        Ok(GamepadDirectionalSource::DpadOnly)
    );
    assert_eq!(
        parse_gamepad_directional_source("left-stick"),
        Ok(GamepadDirectionalSource::LeftStickOnly)
    );
    assert_eq!(
        parse_gamepad_directional_source("both"),
        Ok(GamepadDirectionalSource::DpadAndLeftStick)
    );
    assert!(parse_gamepad_directional_source("stick-only").is_err());

    assert_eq!(
        parse_gamepad_face_layout("east-a"),
        Ok(GamepadFaceLayout::EastASouthB)
    );
    assert_eq!(
        parse_gamepad_face_layout("south-a"),
        Ok(GamepadFaceLayout::SouthAEastB)
    );
    assert!(parse_gamepad_face_layout("north-a").is_err());
}

#[test]
fn gamepad_binding_parsers_cover_supported_buttons_and_slots() {
    assert_eq!(
        parse_gamepad_button_binding("south"),
        Ok(GamepadButtonBinding::South)
    );
    assert_eq!(
        parse_gamepad_button_binding("east"),
        Ok(GamepadButtonBinding::East)
    );
    assert_eq!(
        parse_gamepad_button_binding("back"),
        Ok(GamepadButtonBinding::Back)
    );
    assert_eq!(
        parse_gamepad_button_binding("start"),
        Ok(GamepadButtonBinding::Start)
    );
    assert_eq!(
        parse_gamepad_button_binding("guide"),
        Ok(GamepadButtonBinding::Guide)
    );
    assert_eq!(
        parse_gamepad_button_binding("left-trigger"),
        Ok(GamepadButtonBinding::LeftTrigger)
    );
    assert_eq!(
        parse_gamepad_button_binding("right-trigger"),
        Ok(GamepadButtonBinding::RightTrigger)
    );
    assert_eq!(
        parse_gamepad_button_binding("dpad-up"),
        Ok(GamepadButtonBinding::DPadUp)
    );
    assert_eq!(
        parse_gamepad_button_binding("dpad-down"),
        Ok(GamepadButtonBinding::DPadDown)
    );
    assert_eq!(
        parse_gamepad_button_binding("dpad-left"),
        Ok(GamepadButtonBinding::DPadLeft)
    );
    assert_eq!(
        parse_gamepad_button_binding("left-stick-click"),
        Ok(GamepadButtonBinding::LeftStickClick)
    );
    assert_eq!(
        parse_gamepad_button_binding("right-stick-click"),
        Ok(GamepadButtonBinding::RightStickClick)
    );
    assert_eq!(
        parse_gamepad_button_binding("dpad-right"),
        Ok(GamepadButtonBinding::DPadRight)
    );
    assert_eq!(
        parse_gamepad_button_binding("misc1"),
        Ok(GamepadButtonBinding::Misc1)
    );
    assert!(parse_gamepad_button_binding("touchpad").is_err());

    let mut bindings = GamepadButtonBindings::default();
    apply_gamepad_binding_override(&mut bindings, "up", GamepadButtonBinding::North)
        .expect("known slots should update");
    apply_gamepad_binding_override(&mut bindings, "down", GamepadButtonBinding::South)
        .expect("known slots should update");
    apply_gamepad_binding_override(&mut bindings, "left", GamepadButtonBinding::Back)
        .expect("known slots should update");
    apply_gamepad_binding_override(&mut bindings, "right", GamepadButtonBinding::Guide)
        .expect("known slots should update");
    apply_gamepad_binding_override(&mut bindings, "start", GamepadButtonBinding::Guide)
        .expect("known slots should update");
    assert_eq!(bindings.up, GamepadButtonBinding::North);
    assert_eq!(bindings.down, GamepadButtonBinding::South);
    assert_eq!(bindings.left, GamepadButtonBinding::Back);
    assert_eq!(bindings.right, GamepadButtonBinding::Guide);
    assert_eq!(bindings.start, GamepadButtonBinding::Guide);
    assert!(
        apply_gamepad_binding_override(&mut bindings, "shoulder", GamepadButtonBinding::West)
            .is_err()
    );
}

#[test]
fn text_and_numeric_parsers_trim_and_validate_user_input() {
    assert_eq!(
        parse_non_empty_text("--gamepad-preferred-name", "  Switch Pro  "),
        Ok("Switch Pro".to_string())
    );
    assert!(parse_non_empty_text("--save-key", "   ").is_err());

    assert_eq!(parse_positive_u8("--scale", "6"), Ok(6));
    assert!(parse_positive_u8("--scale", "0").is_err());
    assert!(parse_positive_u8("--scale", "wide").is_err());
    assert_eq!(
        parse_positive_u32("--audio-record-rate", "96000"),
        Ok(96_000)
    );
    assert!(parse_positive_u32("--audio-record-rate", "0").is_err());
    assert!(parse_positive_u32("--audio-record-rate", "wide").is_err());
    assert_eq!(parse_positive_u64("--exit-after-frames", "6"), Ok(6));
    assert!(parse_positive_u64("--exit-after-frames", "0").is_err());
    assert!(parse_positive_u64("--exit-after-frames", "wide").is_err());

    assert_eq!(
        parse_save_key("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)")
            .expect("valid cartridge save keys should parse")
            .as_str(),
        "Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)"
    );
    assert!(parse_save_key("bad/key").is_err());
    assert_eq!(
        parse_audio_recording_stems("all"),
        Ok(ApuRecordedChannel::ALL.to_vec())
    );
    assert_eq!(
        parse_audio_recording_stems("ch2,ch4"),
        Ok(vec![ApuRecordedChannel::Ch2, ApuRecordedChannel::Ch4])
    );
    assert!(parse_audio_recording_stems("").is_err());
    assert!(parse_audio_recording_stems("ch4,ch4").is_err());
    assert!(parse_audio_recording_stems("noise").is_err());
}
