use crate::input::FrontendInputState;
use gb_core::JoypadButton;
use sdl3::keyboard::Scancode;

pub const PLAYER_SLOT_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlayerSlot {
    P1,
    P2,
    P3,
    P4,
}

impl PlayerSlot {
    pub const ALL: [Self; PLAYER_SLOT_COUNT] = [Self::P1, Self::P2, Self::P3, Self::P4];

    pub const fn index(self) -> usize {
        match self {
            Self::P1 => 0,
            Self::P2 => 1,
            Self::P3 => 2,
            Self::P4 => 3,
        }
    }

    pub const fn from_machine_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::P1),
            1 => Some(Self::P2),
            2 => Some(Self::P3),
            3 => Some(Self::P4),
            _ => None,
        }
    }

    pub const fn machine_index(self) -> usize {
        self.index()
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
            Self::P4 => "P4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopDmg07PlayerCount {
    Two,
    Three,
    Four,
}

impl DesktopDmg07PlayerCount {
    pub const fn get(self) -> usize {
        match self {
            Self::Two => 2,
            Self::Three => 3,
            Self::Four => 4,
        }
    }

    pub const fn active_slots(self) -> [Option<PlayerSlot>; PLAYER_SLOT_COUNT] {
        match self {
            Self::Two => [Some(PlayerSlot::P1), Some(PlayerSlot::P2), None, None],
            Self::Three => [
                Some(PlayerSlot::P1),
                Some(PlayerSlot::P2),
                Some(PlayerSlot::P3),
                None,
            ],
            Self::Four => [
                Some(PlayerSlot::P1),
                Some(PlayerSlot::P2),
                Some(PlayerSlot::P3),
                Some(PlayerSlot::P4),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopPlayerSessionKind {
    Single,
    LinkedDmg04TwoPlayer,
    LinkedCgbInfraredTwoPlayer,
    LinkedDmg07 {
        player_count: DesktopDmg07PlayerCount,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerKeyboardProfile {
    ConfiguredJoypad,
    LinkedDmg04P2,
    LinkedDmg07P3,
    LinkedDmg07P4,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAudioPolicy {
    Audible,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerViewPolicy {
    LeftPanel,
    RightPanel,
    BottomLeftPanel,
    BottomRightPanel,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerHostPolicy {
    pub slot: PlayerSlot,
    pub machine_index: Option<usize>,
    pub keyboard_profile: PlayerKeyboardProfile,
    pub audio: PlayerAudioPolicy,
    pub view: PlayerViewPolicy,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerViewSlots {
    pub left: PlayerSlot,
    pub right: Option<PlayerSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerViewLayout {
    pub columns: usize,
    pub rows: usize,
    pub slots: [Option<PlayerSlot>; PLAYER_SLOT_COUNT],
}

pub const fn host_policy_for_slot(
    session_kind: DesktopPlayerSessionKind,
    slot: PlayerSlot,
) -> PlayerHostPolicy {
    match (session_kind, slot) {
        (DesktopPlayerSessionKind::Single, PlayerSlot::P1) => PlayerHostPolicy {
            slot,
            machine_index: Some(0),
            keyboard_profile: PlayerKeyboardProfile::ConfiguredJoypad,
            audio: PlayerAudioPolicy::Audible,
            view: PlayerViewPolicy::LeftPanel,
        },
        (DesktopPlayerSessionKind::LinkedDmg04TwoPlayer, PlayerSlot::P1) => PlayerHostPolicy {
            slot,
            machine_index: Some(0),
            keyboard_profile: PlayerKeyboardProfile::ConfiguredJoypad,
            audio: PlayerAudioPolicy::Audible,
            view: PlayerViewPolicy::LeftPanel,
        },
        (DesktopPlayerSessionKind::LinkedDmg04TwoPlayer, PlayerSlot::P2) => PlayerHostPolicy {
            slot,
            machine_index: Some(1),
            keyboard_profile: PlayerKeyboardProfile::LinkedDmg04P2,
            audio: PlayerAudioPolicy::Muted,
            view: PlayerViewPolicy::RightPanel,
        },
        (DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer, PlayerSlot::P1) => {
            PlayerHostPolicy {
                slot,
                machine_index: Some(0),
                keyboard_profile: PlayerKeyboardProfile::ConfiguredJoypad,
                audio: PlayerAudioPolicy::Audible,
                view: PlayerViewPolicy::LeftPanel,
            }
        }
        (DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer, PlayerSlot::P2) => {
            PlayerHostPolicy {
                slot,
                machine_index: Some(1),
                keyboard_profile: PlayerKeyboardProfile::LinkedDmg04P2,
                audio: PlayerAudioPolicy::Muted,
                view: PlayerViewPolicy::RightPanel,
            }
        }
        (DesktopPlayerSessionKind::LinkedDmg07 { .. }, PlayerSlot::P1) => PlayerHostPolicy {
            slot,
            machine_index: Some(0),
            keyboard_profile: PlayerKeyboardProfile::ConfiguredJoypad,
            audio: PlayerAudioPolicy::Audible,
            view: PlayerViewPolicy::LeftPanel,
        },
        (DesktopPlayerSessionKind::LinkedDmg07 { player_count }, PlayerSlot::P2)
            if player_count.get() >= 2 =>
        {
            PlayerHostPolicy {
                slot,
                machine_index: Some(1),
                keyboard_profile: PlayerKeyboardProfile::LinkedDmg04P2,
                audio: PlayerAudioPolicy::Muted,
                view: PlayerViewPolicy::RightPanel,
            }
        }
        (DesktopPlayerSessionKind::LinkedDmg07 { player_count }, PlayerSlot::P3)
            if player_count.get() >= 3 =>
        {
            PlayerHostPolicy {
                slot,
                machine_index: Some(2),
                keyboard_profile: PlayerKeyboardProfile::LinkedDmg07P3,
                audio: PlayerAudioPolicy::Muted,
                view: PlayerViewPolicy::BottomLeftPanel,
            }
        }
        (DesktopPlayerSessionKind::LinkedDmg07 { player_count }, PlayerSlot::P4)
            if player_count.get() >= 4 =>
        {
            PlayerHostPolicy {
                slot,
                machine_index: Some(3),
                keyboard_profile: PlayerKeyboardProfile::LinkedDmg07P4,
                audio: PlayerAudioPolicy::Muted,
                view: PlayerViewPolicy::BottomRightPanel,
            }
        }
        (_, _) => PlayerHostPolicy {
            slot,
            machine_index: None,
            keyboard_profile: PlayerKeyboardProfile::Disabled,
            audio: PlayerAudioPolicy::Muted,
            view: PlayerViewPolicy::Hidden,
        },
    }
}

pub const fn audio_source_slot(_session_kind: DesktopPlayerSessionKind) -> PlayerSlot {
    PlayerSlot::P1
}

#[cfg(test)]
pub const fn view_slots_for_session(session_kind: DesktopPlayerSessionKind) -> PlayerViewSlots {
    match session_kind {
        DesktopPlayerSessionKind::Single => PlayerViewSlots {
            left: PlayerSlot::P1,
            right: None,
        },
        DesktopPlayerSessionKind::LinkedDmg04TwoPlayer => PlayerViewSlots {
            left: PlayerSlot::P1,
            right: Some(PlayerSlot::P2),
        },
        DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer => PlayerViewSlots {
            left: PlayerSlot::P1,
            right: Some(PlayerSlot::P2),
        },
        DesktopPlayerSessionKind::LinkedDmg07 { .. } => PlayerViewSlots {
            left: PlayerSlot::P1,
            right: Some(PlayerSlot::P2),
        },
    }
}

pub const fn view_layout_for_session(session_kind: DesktopPlayerSessionKind) -> PlayerViewLayout {
    match session_kind {
        DesktopPlayerSessionKind::Single => PlayerViewLayout {
            columns: 1,
            rows: 1,
            slots: [Some(PlayerSlot::P1), None, None, None],
        },
        DesktopPlayerSessionKind::LinkedDmg04TwoPlayer => PlayerViewLayout {
            columns: 2,
            rows: 1,
            slots: [Some(PlayerSlot::P1), Some(PlayerSlot::P2), None, None],
        },
        DesktopPlayerSessionKind::LinkedCgbInfraredTwoPlayer => PlayerViewLayout {
            columns: 2,
            rows: 1,
            slots: [Some(PlayerSlot::P1), Some(PlayerSlot::P2), None, None],
        },
        DesktopPlayerSessionKind::LinkedDmg07 {
            player_count: DesktopDmg07PlayerCount::Two,
        } => PlayerViewLayout {
            columns: 2,
            rows: 1,
            slots: [Some(PlayerSlot::P1), Some(PlayerSlot::P2), None, None],
        },
        DesktopPlayerSessionKind::LinkedDmg07 { player_count } => PlayerViewLayout {
            columns: 2,
            rows: 2,
            slots: player_count.active_slots(),
        },
    }
}

pub struct PlayerInputStates {
    inputs: [FrontendInputState; PLAYER_SLOT_COUNT],
}

impl PlayerInputStates {
    pub fn new() -> Self {
        Self {
            inputs: std::array::from_fn(|_| FrontendInputState::new()),
        }
    }

    pub fn input_mut(&mut self, slot: PlayerSlot) -> &mut FrontendInputState {
        &mut self.inputs[slot.index()]
    }
}

impl Default for PlayerInputStates {
    fn default() -> Self {
        Self::new()
    }
}

pub const LINKED_DMG04_P2_KEYBOARD_BINDINGS: [(JoypadButton, Scancode); 8] = [
    (JoypadButton::Up, Scancode::W),
    (JoypadButton::Down, Scancode::S),
    (JoypadButton::Left, Scancode::A),
    (JoypadButton::Right, Scancode::D),
    (JoypadButton::A, Scancode::X),
    (JoypadButton::B, Scancode::Z),
    (JoypadButton::Select, Scancode::Q),
    (JoypadButton::Start, Scancode::E),
];

pub fn linked_dmg04_p2_button_for_scancode(scancode: Scancode) -> Option<JoypadButton> {
    LINKED_DMG04_P2_KEYBOARD_BINDINGS
        .into_iter()
        .find_map(|(button, binding)| (binding == scancode).then_some(button))
}

pub const LINKED_DMG07_P3_KEYBOARD_BINDINGS: [(JoypadButton, Scancode); 8] = [
    (JoypadButton::Up, Scancode::T),
    (JoypadButton::Down, Scancode::G),
    (JoypadButton::Left, Scancode::F),
    (JoypadButton::Right, Scancode::H),
    (JoypadButton::A, Scancode::B),
    (JoypadButton::B, Scancode::V),
    (JoypadButton::Select, Scancode::R),
    (JoypadButton::Start, Scancode::Y),
];

pub fn linked_dmg07_p3_button_for_scancode(scancode: Scancode) -> Option<JoypadButton> {
    LINKED_DMG07_P3_KEYBOARD_BINDINGS
        .into_iter()
        .find_map(|(button, binding)| (binding == scancode).then_some(button))
}

pub const LINKED_DMG07_P4_KEYBOARD_BINDINGS: [(JoypadButton, Scancode); 8] = [
    (JoypadButton::Up, Scancode::I),
    (JoypadButton::Down, Scancode::K),
    (JoypadButton::Left, Scancode::J),
    (JoypadButton::Right, Scancode::L),
    (JoypadButton::A, Scancode::Comma),
    (JoypadButton::B, Scancode::M),
    (JoypadButton::Select, Scancode::U),
    (JoypadButton::Start, Scancode::O),
];

pub fn linked_dmg07_p4_button_for_scancode(scancode: Scancode) -> Option<JoypadButton> {
    LINKED_DMG07_P4_KEYBOARD_BINDINGS
        .into_iter()
        .find_map(|(button, binding)| (binding == scancode).then_some(button))
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopDmg07PlayerCount, DesktopPlayerSessionKind, PlayerAudioPolicy,
        PlayerKeyboardProfile, PlayerSlot, PlayerViewPolicy, audio_source_slot,
        host_policy_for_slot, linked_dmg04_p2_button_for_scancode,
        linked_dmg07_p3_button_for_scancode, linked_dmg07_p4_button_for_scancode,
        view_layout_for_session, view_slots_for_session,
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
}
