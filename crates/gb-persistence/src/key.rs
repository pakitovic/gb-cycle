mod extension;
mod save_key;

pub use extension::CartridgeSaveFileExtension;
pub use save_key::{CartridgeSaveKey, CartridgeSaveKeyError};

#[cfg(test)]
mod test;
