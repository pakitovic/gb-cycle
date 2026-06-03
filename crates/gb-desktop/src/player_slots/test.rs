use super::{
    DesktopDmg07PlayerCount, DesktopPlayerSessionKind, PlayerAudioPolicy, PlayerKeyboardProfile,
    PlayerSlot, PlayerViewPolicy, audio_source_slot, host_policy_for_slot,
    linked_dmg04_p2_button_for_scancode, linked_dmg07_p3_button_for_scancode,
    linked_dmg07_p4_button_for_scancode, view_layout_for_session, view_slots_for_session,
};
use gb_core::JoypadButton;
use sdl3::keyboard::Scancode;

#[test]
fn player_slots_are_stable_and_indexed_for_future_four_player_sessions() {
    assert_eq!(
        PlayerSlot::ALL,
        [
            PlayerSlot::P1,
            PlayerSlot::P2,
            PlayerSlot::P3,
            PlayerSlot::P4
        ]
    );
    assert_eq!(PlayerSlot::P1.index(), 0);
    assert_eq!(PlayerSlot::P4.machine_index(), 3);
    assert_eq!(PlayerSlot::from_machine_index(2), Some(PlayerSlot::P3));
    assert_eq!(PlayerSlot::from_machine_index(4), None);
    assert_eq!(PlayerSlot::P2.label(), "P2");
}

#[test]
fn host_policy_keeps_single_player_on_p1_only() {
    let p1 = host_policy_for_slot(DesktopPlayerSessionKind::Single, PlayerSlot::P1);
    assert_eq!(p1.machine_index, Some(0));
    assert_eq!(p1.keyboard_profile, PlayerKeyboardProfile::ConfiguredJoypad);
    assert_eq!(p1.audio, PlayerAudioPolicy::Audible);
    assert_eq!(p1.view, PlayerViewPolicy::LeftPanel);

    let p2 = host_policy_for_slot(DesktopPlayerSessionKind::Single, PlayerSlot::P2);
    assert_eq!(p2.machine_index, None);
    assert_eq!(p2.keyboard_profile, PlayerKeyboardProfile::Disabled);
    assert_eq!(p2.audio, PlayerAudioPolicy::Muted);
    assert_eq!(p2.view, PlayerViewPolicy::Hidden);
}

#[test]
fn host_policy_maps_dmg04_to_p1_and_p2_without_enabling_future_slots() {
    let p1 = host_policy_for_slot(
        DesktopPlayerSessionKind::LinkedDmg04TwoPlayer,
        PlayerSlot::P1,
    );
    let p2 = host_policy_for_slot(
        DesktopPlayerSessionKind::LinkedDmg04TwoPlayer,
        PlayerSlot::P2,
    );
    let p3 = host_policy_for_slot(
        DesktopPlayerSessionKind::LinkedDmg04TwoPlayer,
        PlayerSlot::P3,
    );

    assert_eq!(p1.machine_index, Some(0));
    assert_eq!(p1.keyboard_profile, PlayerKeyboardProfile::ConfiguredJoypad);
    assert_eq!(p1.audio, PlayerAudioPolicy::Audible);
    assert_eq!(p1.view, PlayerViewPolicy::LeftPanel);

    assert_eq!(p2.machine_index, Some(1));
    assert_eq!(p2.keyboard_profile, PlayerKeyboardProfile::LinkedDmg04P2);
    assert_eq!(p2.audio, PlayerAudioPolicy::Muted);
    assert_eq!(p2.view, PlayerViewPolicy::RightPanel);

    assert_eq!(p3.machine_index, None);
    assert_eq!(p3.view, PlayerViewPolicy::Hidden);
}

#[test]
fn host_policy_maps_cgb_ir_like_a_two_panel_two_player_session() {
    let p1 = host_policy_for_slot(
        DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer,
        PlayerSlot::P1,
    );
    let p2 = host_policy_for_slot(
        DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer,
        PlayerSlot::P2,
    );
    let p3 = host_policy_for_slot(
        DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer,
        PlayerSlot::P3,
    );

    assert_eq!(p1.machine_index, Some(0));
    assert_eq!(p1.keyboard_profile, PlayerKeyboardProfile::ConfiguredJoypad);
    assert_eq!(p1.audio, PlayerAudioPolicy::Audible);
    assert_eq!(p1.view, PlayerViewPolicy::LeftPanel);

    assert_eq!(p2.machine_index, Some(1));
    assert_eq!(p2.keyboard_profile, PlayerKeyboardProfile::LinkedDmg04P2);
    assert_eq!(p2.audio, PlayerAudioPolicy::Muted);
    assert_eq!(p2.view, PlayerViewPolicy::RightPanel);

    assert_eq!(p3.machine_index, None);
    assert_eq!(p3.view, PlayerViewPolicy::Hidden);
}

#[test]
fn host_policy_maps_dmg07_slots_without_enabling_inactive_players() {
    let session = DesktopPlayerSessionKind::LinkedDmg07 {
        player_count: DesktopDmg07PlayerCount::Three,
    };
    let p1 = host_policy_for_slot(session, PlayerSlot::P1);
    let p2 = host_policy_for_slot(session, PlayerSlot::P2);
    let p3 = host_policy_for_slot(session, PlayerSlot::P3);
    let p4 = host_policy_for_slot(session, PlayerSlot::P4);

    assert_eq!(p1.machine_index, Some(0));
    assert_eq!(p1.keyboard_profile, PlayerKeyboardProfile::ConfiguredJoypad);
    assert_eq!(p1.audio, PlayerAudioPolicy::Audible);
    assert_eq!(p1.view, PlayerViewPolicy::LeftPanel);

    assert_eq!(p2.machine_index, Some(1));
    assert_eq!(p2.keyboard_profile, PlayerKeyboardProfile::LinkedDmg04P2);
    assert_eq!(p2.audio, PlayerAudioPolicy::Muted);
    assert_eq!(p2.view, PlayerViewPolicy::RightPanel);

    assert_eq!(p3.machine_index, Some(2));
    assert_eq!(p3.keyboard_profile, PlayerKeyboardProfile::LinkedDmg07P3);
    assert_eq!(p3.audio, PlayerAudioPolicy::Muted);
    assert_eq!(p3.view, PlayerViewPolicy::BottomLeftPanel);

    assert_eq!(p4.machine_index, None);
    assert_eq!(p4.keyboard_profile, PlayerKeyboardProfile::Disabled);
    assert_eq!(p4.view, PlayerViewPolicy::Hidden);
}

#[test]
fn default_audio_and_view_policy_stay_frontend_owned() {
    assert_eq!(
        audio_source_slot(DesktopPlayerSessionKind::Single),
        PlayerSlot::P1
    );
    assert_eq!(
        audio_source_slot(DesktopPlayerSessionKind::LinkedDmg04TwoPlayer),
        PlayerSlot::P1
    );

    assert_eq!(
        view_slots_for_session(DesktopPlayerSessionKind::Single).right,
        None
    );
    assert_eq!(
        view_slots_for_session(DesktopPlayerSessionKind::LinkedDmg04TwoPlayer).right,
        Some(PlayerSlot::P2)
    );
    assert_eq!(
        view_slots_for_session(DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer).right,
        Some(PlayerSlot::P2)
    );
    let cgb_ir_layout =
        view_layout_for_session(DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer);
    assert_eq!(cgb_ir_layout.columns, 2);
    assert_eq!(cgb_ir_layout.rows, 1);
    assert_eq!(
        cgb_ir_layout.slots,
        [Some(PlayerSlot::P1), Some(PlayerSlot::P2), None, None]
    );

    let four_player_layout = view_layout_for_session(DesktopPlayerSessionKind::LinkedDmg07 {
        player_count: DesktopDmg07PlayerCount::Four,
    });
    assert_eq!(four_player_layout.columns, 2);
    assert_eq!(four_player_layout.rows, 2);
    assert_eq!(
        four_player_layout.slots,
        [
            Some(PlayerSlot::P1),
            Some(PlayerSlot::P2),
            Some(PlayerSlot::P3),
            Some(PlayerSlot::P4)
        ]
    );
}

#[test]
fn linked_dmg04_p2_keyboard_profile_is_explicit() {
    assert_eq!(
        linked_dmg04_p2_button_for_scancode(Scancode::W),
        Some(JoypadButton::Up)
    );
    assert_eq!(
        linked_dmg04_p2_button_for_scancode(Scancode::E),
        Some(JoypadButton::Start)
    );
    assert_eq!(
        linked_dmg04_p2_button_for_scancode(Scancode::X),
        Some(JoypadButton::A)
    );
    assert_eq!(
        linked_dmg04_p2_button_for_scancode(Scancode::Z),
        Some(JoypadButton::B)
    );
    assert_eq!(linked_dmg04_p2_button_for_scancode(Scancode::V), None);
    assert_eq!(
        linked_dmg07_p3_button_for_scancode(Scancode::T),
        Some(JoypadButton::Up)
    );
    assert_eq!(
        linked_dmg07_p3_button_for_scancode(Scancode::Y),
        Some(JoypadButton::Start)
    );
    assert_eq!(
        linked_dmg07_p3_button_for_scancode(Scancode::B),
        Some(JoypadButton::A)
    );
    assert_eq!(
        linked_dmg07_p4_button_for_scancode(Scancode::I),
        Some(JoypadButton::Up)
    );
    assert_eq!(
        linked_dmg07_p4_button_for_scancode(Scancode::O),
        Some(JoypadButton::Start)
    );
    assert_eq!(
        linked_dmg07_p4_button_for_scancode(Scancode::Comma),
        Some(JoypadButton::A)
    );
    assert_eq!(
        linked_dmg07_p4_button_for_scancode(Scancode::M),
        Some(JoypadButton::B)
    );
}
