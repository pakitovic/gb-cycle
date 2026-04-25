use crate::boot::StartupMemoryPolicy;

use super::WRAM_LEN;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct WramDomain {
    #[serde(with = "serde_big_array::BigArray")]
    bytes: [u8; WRAM_LEN],
}

impl WramDomain {
    pub(crate) fn new() -> Self {
        Self {
            bytes: [0; WRAM_LEN],
        }
    }

    pub(crate) fn apply_startup_memory_policy(&mut self, policy: StartupMemoryPolicy) {
        policy.initialize_wram(&mut self.bytes);
    }

    pub(crate) fn read(&self, address: u16) -> u8 {
        self.bytes[self.index(address)]
    }

    pub(crate) fn write(&mut self, address: u16, value: u8) {
        let index = self.index(address);
        self.bytes[index] = value;
    }

    fn index(&self, address: u16) -> usize {
        match address {
            0xC000..=0xDFFF => (address - 0xC000) as usize,
            0xE000..=0xFDFF => (address - 0xE000) as usize,
            _ => panic!("address {address:#06X} does not map to WRAM storage"),
        }
    }
}
