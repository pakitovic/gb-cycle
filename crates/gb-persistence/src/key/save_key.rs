use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CartridgeSaveKey(String);

impl CartridgeSaveKey {
    pub fn new(key: impl Into<String>) -> Result<Self, CartridgeSaveKeyError> {
        let key = key.into();
        if key.is_empty() {
            return Err(CartridgeSaveKeyError::Empty);
        }

        for (index, character) in key.chars().enumerate() {
            if !is_portable_save_key_character(character) {
                return Err(CartridgeSaveKeyError::InvalidCharacter { index, character });
            }
        }

        Ok(Self(key))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_portable_save_key_character(character: char) -> bool {
    !character.is_control()
        && !matches!(
            character,
            '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
        )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CartridgeSaveKeyError {
    Empty,
    InvalidCharacter { index: usize, character: char },
}

impl fmt::Display for CartridgeSaveKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "save key must not be empty"),
            Self::InvalidCharacter { index, character } => write!(
                f,
                "save key contains invalid character `{character}` at index {index}"
            ),
        }
    }
}

impl std::error::Error for CartridgeSaveKeyError {}
