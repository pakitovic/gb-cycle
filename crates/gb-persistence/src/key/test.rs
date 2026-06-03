use super::*;

#[test]
fn save_key_rejects_invalid_characters_and_empty_values() {
    assert_eq!(CartridgeSaveKey::new(""), Err(CartridgeSaveKeyError::Empty));
    assert_eq!(
        CartridgeSaveKey::new("phase/6"),
        Err(CartridgeSaveKeyError::InvalidCharacter {
            index: 5,
            character: '/',
        })
    );
    assert!(CartridgeSaveKey::new("phase6_save").is_ok());
}
