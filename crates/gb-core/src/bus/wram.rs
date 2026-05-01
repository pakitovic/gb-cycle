use crate::boot::StartupMemoryPolicy;
use crate::model::ConsoleModel;

use super::{CGB_WRAM_LEN, DMG_WRAM_LEN};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct WramDomain {
    console_model: ConsoleModel,
    selected_bank_register: u8,
    #[serde(with = "serde_big_array::BigArray")]
    bytes: [u8; CGB_WRAM_LEN],
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct WramSaveState {
    console_model: ConsoleModel,
    selected_bank_register: u8,
    bytes: Vec<u8>,
}

impl WramSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.bytes.len()
    }
}

impl WramDomain {
    pub(crate) fn new_for_model(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            selected_bank_register: 0,
            bytes: [0; CGB_WRAM_LEN],
        }
    }

    pub(crate) fn apply_startup_memory_policy(&mut self, policy: StartupMemoryPolicy) {
        policy.initialize_wram(self.debug_bytes_mut());
    }

    pub(crate) fn read(&self, address: u16) -> u8 {
        self.bytes[self.index(address)]
    }

    pub(crate) fn write(&mut self, address: u16, value: u8) {
        let index = self.index(address);
        self.bytes[index] = value;
    }

    pub(crate) fn debug_bytes(&self) -> &[u8] {
        let len = if self.console_model.is_cgb_family() {
            CGB_WRAM_LEN
        } else {
            DMG_WRAM_LEN
        };
        &self.bytes[..len]
    }

    fn debug_bytes_mut(&mut self) -> &mut [u8] {
        let len = if self.console_model.is_cgb_family() {
            CGB_WRAM_LEN
        } else {
            DMG_WRAM_LEN
        };
        &mut self.bytes[..len]
    }

    pub(crate) fn capture_save_state(&self) -> WramSaveState {
        WramSaveState {
            console_model: self.console_model,
            selected_bank_register: self.selected_bank_register,
            bytes: self.debug_bytes().to_vec(),
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &WramSaveState) {
        self.console_model = state.console_model;
        self.selected_bank_register = state.selected_bank_register & 0x07;
        self.bytes.fill(0);
        let copy_len = state.bytes.len().min(self.bytes.len());
        self.bytes[..copy_len].copy_from_slice(&state.bytes[..copy_len]);
    }

    pub(crate) fn selected_wram_bank(&self) -> u8 {
        match self.selected_bank_register & 0x07 {
            0 => 1,
            bank => bank,
        }
    }

    pub(crate) fn read_svbk(&self) -> u8 {
        0xF8 | (self.selected_bank_register & 0x07)
    }

    pub(crate) fn write_svbk(&mut self, value: u8) {
        if self.console_model.is_cgb_family() {
            self.selected_bank_register = value & 0x07;
        }
    }

    pub(crate) fn reset_bank_select(&mut self) {
        self.selected_bank_register = 0;
    }

    fn index(&self, address: u16) -> usize {
        match address {
            0xC000..=0xCFFF => (address - 0xC000) as usize,
            0xD000..=0xDFFF => self.switchable_bank_index(address - 0xD000),
            0xE000..=0xEFFF => (address - 0xE000) as usize,
            0xF000..=0xFDFF => self.switchable_bank_index(address - 0xF000),
            _ => panic!("address {address:#06X} does not map to WRAM storage"),
        }
    }

    fn switchable_bank_index(&self, offset: u16) -> usize {
        if self.console_model.is_cgb_family() {
            usize::from(self.selected_wram_bank()) * 0x1000 + usize::from(offset)
        } else {
            0x1000 + usize::from(offset)
        }
    }
}
