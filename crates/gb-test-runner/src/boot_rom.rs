mod assets;

#[cfg(test)]
mod test;

#[cfg(test)]
pub(crate) use assets::{BootRomLoadError, asset_for_profile};
pub(crate) use assets::{BootRomProfile, load_verified_boot_rom_assets};
