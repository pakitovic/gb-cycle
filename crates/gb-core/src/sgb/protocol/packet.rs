use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbHostStatus {
    Disabled,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SgbCommandAcceptance {
    Disabled,
    AwaitingCartridgeHeader,
    RejectedByHeader,
    Accepted,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbJoypLineState {
    #[default]
    Idle,
    Start,
    Zero,
    One,
    Invalid,
}

impl SgbJoypLineState {
    pub(in crate::sgb) const fn from_joyp_value(value: u8) -> Self {
        match value & JOYP_SELECT_BITS_MASK {
            SGB_JOYP_IDLE_BITS => Self::Idle,
            SGB_JOYP_START_BITS => Self::Start,
            SGB_JOYP_ZERO_BITS => Self::Zero,
            SGB_JOYP_ONE_BITS => Self::One,
            _ => Self::Invalid,
        }
    }

    pub(in crate::sgb) const fn data_bit(self) -> Option<u8> {
        match self {
            Self::Zero => Some(0),
            Self::One => Some(1),
            Self::Idle | Self::Start | Self::Invalid => None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbPacketTransportPhase {
    #[default]
    Idle,
    StartPending,
    Receiving,
    DataPending,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SgbPacketTraceStatus {
    #[default]
    None,
    Complete,
    RejectedByHeader,
    RejectedWhileBusy,
    SuppressedByIcon,
    InvalidPacketLength,
    InvalidStopBit,
    IncompleteReset,
    OrphanDataPulse,
    ConflictingPulse,
}
