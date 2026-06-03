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
mod test;
