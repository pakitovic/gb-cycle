mod conversion;
mod linear;
mod mbc2;
mod mbc3;
mod mbc6;
mod types;

pub(crate) use conversion::external_save_error_allows_internal_fallback;
pub use conversion::{
    encode_external_cartridge_save, export_external_cartridge_save, import_external_cartridge_save,
};
pub use types::{ExternalSaveError, ExternalSaveExportFormat, ExternalSaveLengthExpectation};

#[cfg(test)]
mod test;
