use gb_persistence::{CartridgeSaveKey, CartridgeSaveKeyError};
use std::path::Path;

pub(crate) fn derive_save_key(rom_path: &Path) -> Result<CartridgeSaveKey, String> {
    let stem = rom_path
        .file_stem()
        .or_else(|| rom_path.file_name())
        .ok_or_else(|| {
            format!(
                "could not derive a save key from ROM path {}; use --save-key instead",
                rom_path.display()
            )
        })?
        .to_string_lossy()
        .into_owned();
    parse_save_key(&stem)
        .map_err(|error| format!("could not use ROM stem {stem:?} as save key: {error}"))
}

pub(crate) fn parse_save_key(key: &str) -> Result<CartridgeSaveKey, String> {
    CartridgeSaveKey::new(key).map_err(format_save_key_error)
}

pub(crate) fn format_save_key_error(error: CartridgeSaveKeyError) -> String {
    error.to_string()
}
