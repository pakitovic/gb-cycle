use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbRealBootAsset {
    SgbBoot,
    Sgb2Boot,
}

impl SgbRealBootAsset {
    pub const fn from_profile(profile: SgbHostProfile) -> Self {
        match profile {
            SgbHostProfile::SgbNtsc | SgbHostProfile::SgbPal => Self::SgbBoot,
            SgbHostProfile::Sgb2Ntsc => Self::Sgb2Boot,
        }
    }

    pub const fn filename(self) -> &'static str {
        match self {
            Self::SgbBoot => "sgb_boot.bin",
            Self::Sgb2Boot => "sgb2_boot.bin",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostAudioRequest {
    Sound(SgbSoundRequest),
    SoundTransfer(SgbSoundTransferRequest),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbSnesHostRequest {
    DataSend(SgbDataSendRequest),
    DataTransfer(SgbDataTransferRequest),
    Jump(SgbJumpRequest),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbSoundEffectControl {
    pub code: u8,
    pub pitch: u8,
    pub volume: u8,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbSoundRequest {
    pub effect_a: SgbSoundEffectControl,
    pub effect_b: SgbSoundEffectControl,
    pub music_score: u8,
    pub raw_attributes: u8,
}

impl SgbSoundRequest {
    pub(in crate::sgb) const fn from_packet(bytes: &[u8; SGB_PACKET_BYTES]) -> Self {
        let raw_attributes = bytes[3];
        Self {
            effect_a: SgbSoundEffectControl {
                code: bytes[1],
                pitch: raw_attributes & 0x03,
                volume: (raw_attributes >> 2) & 0x03,
            },
            effect_b: SgbSoundEffectControl {
                code: bytes[2],
                pitch: (raw_attributes >> 4) & 0x03,
                volume: (raw_attributes >> 6) & 0x03,
            },
            music_score: bytes[4],
            raw_attributes,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbApuRamAddress {
    pub address: u16,
}

impl SgbApuRamAddress {
    pub const fn new(address: u16) -> Self {
        Self { address }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbSoundTransferPacket {
    Data {
        size: u16,
        destination: SgbApuRamAddress,
    },
    Jump {
        address: SgbApuRamAddress,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SgbSoundTransferRequest {
    pub first_packet: SgbSoundTransferPacket,
    pub payload_bytes: u32,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbSnesAddress {
    pub bank: u8,
    pub address: u16,
}

impl SgbSnesAddress {
    pub const fn new(bank: u8, address: u16) -> Self {
        Self { bank, address }
    }

    pub(in crate::sgb) const fn from_packet_bytes(low: u8, high: u8, bank: u8) -> Self {
        Self {
            bank,
            address: u16::from_le_bytes([low, high]),
        }
    }

    pub const fn raw24(self) -> u32 {
        (self.bank as u32) << 16 | self.address as u32
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbDataSendRequest {
    pub destination: SgbSnesAddress,
    pub declared_len: u8,
    pub data: [u8; SGB_DATA_SND_INLINE_BYTES],
}

impl SgbDataSendRequest {
    pub(in crate::sgb) const fn from_packet(bytes: &[u8; SGB_PACKET_BYTES]) -> Self {
        let mut data = [0; SGB_DATA_SND_INLINE_BYTES];
        let mut index = 0;
        while index < SGB_DATA_SND_INLINE_BYTES {
            data[index] = bytes[5 + index];
            index += 1;
        }
        Self {
            destination: SgbSnesAddress::from_packet_bytes(bytes[1], bytes[2], bytes[3]),
            declared_len: bytes[4],
            data,
        }
    }

    pub const fn payload_len(self) -> usize {
        if self.declared_len as usize > SGB_DATA_SND_INLINE_BYTES {
            SGB_DATA_SND_INLINE_BYTES
        } else {
            self.declared_len as usize
        }
    }

    pub fn payload(&self) -> &[u8] {
        &self.data[..self.payload_len()]
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbDataTransferRequest {
    pub destination: SgbSnesAddress,
    pub payload_bytes: u32,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct SgbJumpRequest {
    pub program_counter: SgbSnesAddress,
    pub nmi_handler: SgbSnesAddress,
}

impl SgbJumpRequest {
    pub(in crate::sgb) const fn from_packet(bytes: &[u8; SGB_PACKET_BYTES]) -> Self {
        Self {
            program_counter: SgbSnesAddress::from_packet_bytes(bytes[1], bytes[2], bytes[3]),
            nmi_handler: SgbSnesAddress::from_packet_bytes(bytes[4], bytes[5], bytes[6]),
        }
    }
}

impl SgbSoundTransferRequest {
    pub(in crate::sgb) fn from_vram_transfer_payload(payload: &SgbVramTransferBuffer) -> Self {
        let size = u16::from_le_bytes([
            payload.bytes.first().copied().unwrap_or(0),
            payload.bytes.get(1).copied().unwrap_or(0),
        ]);
        let address = SgbApuRamAddress::new(u16::from_le_bytes([
            payload.bytes.get(2).copied().unwrap_or(0),
            payload.bytes.get(3).copied().unwrap_or(0),
        ]));
        Self {
            first_packet: if size == 0 {
                SgbSoundTransferPacket::Jump { address }
            } else {
                SgbSoundTransferPacket::Data {
                    size,
                    destination: address,
                }
            },
            payload_bytes: payload.bytes.len() as u32,
        }
    }
}
