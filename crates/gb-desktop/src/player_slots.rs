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

    #[cfg(test)]
    pub const fn from_machine_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::P1),
            1 => Some(Self::P2),
            2 => Some(Self::P3),
            3 => Some(Self::P4),
            _ => None,
        }
    }

    #[cfg(test)]
    pub const fn machine_index(self) -> usize {
        self.index()
    }

    #[cfg(test)]
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
pub enum DesktopPlayerSessionKind {
    Single,
    LinkedDmg04TwoPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerKeyboardProfile {
    ConfiguredPrimary,
    LinkedDmg04Secondary,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAudioPolicy {
    Audible,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerViewPolicy {
    PrimaryPanel,
    SecondaryPanel,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerViewSlots {
    pub primary: PlayerSlot,
    pub secondary: Option<PlayerSlot>,
}

pub const fn host_policy_for_slot(
    session_kind: DesktopPlayerSessionKind,
    slot: PlayerSlot,
) -> PlayerHostPolicy {
    match (session_kind, slot) {
        (DesktopPlayerSessionKind::Single, PlayerSlot::P1) => PlayerHostPolicy {
            slot,
            machine_index: Some(0),
            keyboard_profile: PlayerKeyboardProfile::ConfiguredPrimary,
            audio: PlayerAudioPolicy::Audible,
            view: PlayerViewPolicy::PrimaryPanel,
        },
        (DesktopPlayerSessionKind::LinkedDmg04TwoPlayer, PlayerSlot::P1) => PlayerHostPolicy {
            slot,
            machine_index: Some(0),
            keyboard_profile: PlayerKeyboardProfile::ConfiguredPrimary,
            audio: PlayerAudioPolicy::Audible,
            view: PlayerViewPolicy::PrimaryPanel,
        },
        (DesktopPlayerSessionKind::LinkedDmg04TwoPlayer, PlayerSlot::P2) => PlayerHostPolicy {
            slot,
            machine_index: Some(1),
            keyboard_profile: PlayerKeyboardProfile::LinkedDmg04Secondary,
            audio: PlayerAudioPolicy::Muted,
            view: PlayerViewPolicy::SecondaryPanel,
        },
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

pub const fn view_slots_for_session(session_kind: DesktopPlayerSessionKind) -> PlayerViewSlots {
    match session_kind {
        DesktopPlayerSessionKind::Single => PlayerViewSlots {
            primary: PlayerSlot::P1,
            secondary: None,
        },
        DesktopPlayerSessionKind::LinkedDmg04TwoPlayer => PlayerViewSlots {
            primary: PlayerSlot::P1,
            secondary: Some(PlayerSlot::P2),
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
    (JoypadButton::A, Scancode::V),
    (JoypadButton::B, Scancode::C),
    (JoypadButton::Select, Scancode::Q),
    (JoypadButton::Start, Scancode::E),
];

pub fn linked_dmg04_p2_button_for_scancode(scancode: Scancode) -> Option<JoypadButton> {
    LINKED_DMG04_P2_KEYBOARD_BINDINGS
        .into_iter()
        .find_map(|(button, binding)| (binding == scancode).then_some(button))
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopPlayerSessionKind, PlayerAudioPolicy, PlayerKeyboardProfile, PlayerSlot,
        PlayerViewPolicy, audio_source_slot, host_policy_for_slot,
        linked_dmg04_p2_button_for_scancode, view_slots_for_session,
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
        assert_eq!(
            p1.keyboard_profile,
            PlayerKeyboardProfile::ConfiguredPrimary
        );
        assert_eq!(p1.audio, PlayerAudioPolicy::Audible);
        assert_eq!(p1.view, PlayerViewPolicy::PrimaryPanel);

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
        assert_eq!(
            p1.keyboard_profile,
            PlayerKeyboardProfile::ConfiguredPrimary
        );
        assert_eq!(p1.audio, PlayerAudioPolicy::Audible);
        assert_eq!(p1.view, PlayerViewPolicy::PrimaryPanel);

        assert_eq!(p2.machine_index, Some(1));
        assert_eq!(
            p2.keyboard_profile,
            PlayerKeyboardProfile::LinkedDmg04Secondary
        );
        assert_eq!(p2.audio, PlayerAudioPolicy::Muted);
        assert_eq!(p2.view, PlayerViewPolicy::SecondaryPanel);

        assert_eq!(p3.machine_index, None);
        assert_eq!(p3.view, PlayerViewPolicy::Hidden);
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
            view_slots_for_session(DesktopPlayerSessionKind::Single).secondary,
            None
        );
        assert_eq!(
            view_slots_for_session(DesktopPlayerSessionKind::LinkedDmg04TwoPlayer).secondary,
            Some(PlayerSlot::P2)
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
        assert_eq!(linked_dmg04_p2_button_for_scancode(Scancode::Z), None);
    }
}
