mod contract;
mod error;
mod filesystem;
mod in_memory;

pub use contract::CartridgeSaveBackend;
pub use error::CartridgeSaveBackendError;
pub use filesystem::FilesystemCartridgeSaveBackend;
pub use in_memory::InMemoryCartridgeSaveBackend;

#[cfg(test)]
mod test;
