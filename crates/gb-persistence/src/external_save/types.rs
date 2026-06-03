use gb_core::CartridgePersistenceProfile;
use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExternalSaveExportFormat {
    #[default]
    Mgba,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalSaveError {
    UnsupportedPersistentState {
        state_kind: &'static str,
    },
    UnsupportedPersistenceProfile {
        profile: CartridgePersistenceProfile,
    },
    StateProfileMismatch {
        state_kind: &'static str,
        profile: CartridgePersistenceProfile,
    },
    InvalidLength {
        context: &'static str,
        expected: ExternalSaveLengthExpectation,
        actual: usize,
    },
    UnsupportedStateShape {
        state_kind: &'static str,
        reason: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalSaveLengthExpectation {
    Exact(usize),
    Either { first: usize, second: usize },
}

impl fmt::Display for ExternalSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPersistentState { state_kind } => {
                write!(f, "external .sav conversion does not support {state_kind}")
            }
            Self::UnsupportedPersistenceProfile { profile } => {
                write!(
                    f,
                    "external .sav conversion does not support persistence profile {profile:?}"
                )
            }
            Self::StateProfileMismatch {
                state_kind,
                profile,
            } => write!(
                f,
                "persistent state {state_kind} does not match cartridge persistence profile {profile:?}"
            ),
            Self::InvalidLength {
                context,
                expected,
                actual,
            } => match expected {
                ExternalSaveLengthExpectation::Exact(expected) => write!(
                    f,
                    "invalid external .sav length for {context}: expected {expected} bytes, got {actual}"
                ),
                ExternalSaveLengthExpectation::Either { first, second } => write!(
                    f,
                    "invalid external .sav length for {context}: expected {first} or {second} bytes, got {actual}"
                ),
            },
            Self::UnsupportedStateShape { state_kind, reason } => {
                write!(
                    f,
                    "external .sav conversion does not support {state_kind}: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for ExternalSaveError {}
