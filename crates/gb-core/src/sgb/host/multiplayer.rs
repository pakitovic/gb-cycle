use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbMultiplayerState {
    pub player_count: u8,
    pub selected_player: u8,
    pub player_pressed_masks: [u8; SGB_CONTROLLER_COUNT],
    pub last_mlt_req_control: u8,
    pub mlt_req_count: u64,
    pub player_cycle_count: u64,
}

impl SgbHost {
    pub const fn selected_player_pressed_mask(&self) -> u8 {
        self.multiplayer.selected_player_pressed_mask()
    }

    pub fn set_player_pressed_mask(&mut self, player: u8, pressed_mask: u8) -> bool {
        self.multiplayer
            .set_player_pressed_mask(player, pressed_mask)
    }

    pub fn set_player_pressed_masks(&mut self, pressed_masks: [u8; SGB_CONTROLLER_COUNT]) -> bool {
        self.multiplayer.set_player_pressed_masks(pressed_masks)
    }

    pub fn set_player_button_pressed(
        &mut self,
        player: u8,
        button: JoypadButton,
        pressed: bool,
    ) -> bool {
        self.multiplayer
            .set_player_button_pressed(player, button, pressed)
    }

    pub fn set_player_palette_override(&mut self, palette: SgbScreenPalette) -> bool {
        if !self.host_platform.is_sgb() {
            return false;
        }
        self.video.set_player_palette_override(palette)
    }

    pub fn clear_player_palette_override(&mut self) -> bool {
        if !self.host_platform.is_sgb() {
            return false;
        }
        self.video.clear_player_palette_override()
    }

    pub const fn player_pressed_masks(&self) -> [u8; SGB_CONTROLLER_COUNT] {
        self.multiplayer.player_pressed_masks
    }

    pub fn joyp_read_value(&self, value: u8) -> u8 {
        if self.host_platform.is_sgb() {
            self.multiplayer.joyp_read_value(value)
        } else {
            value
        }
    }
}

impl SgbMultiplayerState {
    pub(in crate::sgb) const fn default_for_active_host(active: bool) -> Self {
        Self {
            player_count: if active { 1 } else { 0 },
            selected_player: if active { 1 } else { 0 },
            player_pressed_masks: [0; SGB_CONTROLLER_COUNT],
            last_mlt_req_control: 0,
            mlt_req_count: 0,
            player_cycle_count: 0,
        }
    }

    pub(in crate::sgb) fn apply_mlt_req_command(&mut self, bytes: &[u8; SGB_PACKET_BYTES]) {
        if self.player_count == 0 {
            return;
        }

        let control = bytes[1] & 0x03;
        let player_count = match control {
            0 => 1,
            1 => 2,
            2 => 3,
            _ => 4,
        };
        let current_player_index = self.selected_player_index();
        let selected_player_index = if control == 2 {
            (current_player_index + 1) & 0x02
        } else {
            current_player_index & (player_count - 1)
        };

        self.player_count = player_count;
        self.selected_player = selected_player_index + 1;
        self.last_mlt_req_control = control;
        self.mlt_req_count = self.mlt_req_count.saturating_add(1);
    }

    pub(in crate::sgb::host) fn observe_joyp_write(
        &mut self,
        previous_line_state: SgbJoypLineState,
        value: u8,
    ) {
        if !self.cycles_players_on_p15_rise() {
            return;
        }

        let previous_p15_low = matches!(
            previous_line_state,
            SgbJoypLineState::Start | SgbJoypLineState::One
        );
        let current_p15_high = value & 0x20 != 0;
        if previous_p15_low && current_p15_high {
            self.cycle_selected_player();
        }
    }

    pub(in crate::sgb) fn cycle_selected_player(&mut self) {
        let player_count = self.player_count.min(SGB_CONTROLLER_COUNT as u8);
        if player_count == 0 {
            self.selected_player = 0;
            return;
        }

        let selected_player_index = (self.selected_player_index() + 1) & (player_count - 1);
        self.selected_player = selected_player_index + 1;
        self.player_cycle_count = self.player_cycle_count.saturating_add(1);
    }

    const fn cycles_players_on_p15_rise(&self) -> bool {
        self.player_count != 0 && self.player_count & 0x01 == 0
    }

    pub(in crate::sgb) const fn selected_player_index(&self) -> u8 {
        if self.selected_player == 0 {
            0
        } else if self.selected_player > SGB_CONTROLLER_COUNT as u8 {
            SGB_CONTROLLER_COUNT as u8 - 1
        } else {
            self.selected_player - 1
        }
    }

    pub const fn selected_player_pressed_mask(&self) -> u8 {
        if self.player_count == 0 {
            0
        } else {
            self.player_pressed_masks[self.selected_player_index() as usize]
        }
    }

    pub fn set_player_pressed_mask(&mut self, player: u8, pressed_mask: u8) -> bool {
        let Some(player_index) = player_index(player) else {
            return false;
        };
        if self.player_pressed_masks[player_index] == pressed_mask {
            return false;
        }

        self.player_pressed_masks[player_index] = pressed_mask;
        true
    }

    pub fn set_player_pressed_masks(&mut self, pressed_masks: [u8; SGB_CONTROLLER_COUNT]) -> bool {
        if self.player_pressed_masks == pressed_masks {
            return false;
        }

        self.player_pressed_masks = pressed_masks;
        true
    }

    pub fn set_player_button_pressed(
        &mut self,
        player: u8,
        button: JoypadButton,
        pressed: bool,
    ) -> bool {
        let Some(player_index) = player_index(player) else {
            return false;
        };
        let bit = button_mask(button);
        let previous_mask = self.player_pressed_masks[player_index];
        let pressed_mask = if pressed {
            previous_mask | bit
        } else {
            previous_mask & !bit
        };
        if pressed_mask == previous_mask {
            return false;
        }

        self.player_pressed_masks[player_index] = pressed_mask;
        true
    }

    fn joyp_read_value(self, value: u8) -> u8 {
        if self.player_count > 1 && value & JOYP_SELECT_BITS_MASK == JOYP_SELECT_BITS_MASK {
            (value & 0xF0) | (0x0F - self.selected_player_index())
        } else {
            value
        }
    }
}

impl Default for SgbMultiplayerState {
    fn default() -> Self {
        Self::default_for_active_host(false)
    }
}

const fn player_index(player: u8) -> Option<usize> {
    if player == 0 || player > SGB_CONTROLLER_COUNT as u8 {
        None
    } else {
        Some((player - 1) as usize)
    }
}
