use super::*;
use crate::external_save::{ExternalSaveError, ExternalSaveLengthExpectation};
use crate::format::{
    SAVE_FILE_EXTENSION, SAVE_FILE_EXTENSION_P2, SAVE_FILE_EXTENSION_P3, SAVE_FILE_EXTENSION_P4,
};
use crate::hardware::HardwarePersistenceError;
use crate::key::CartridgeSaveKey;
use crate::key::{CartridgeSaveFileExtension, CartridgeSaveKeyError};
use gb_core::{
    CartridgePersistenceProfile, CartridgePersistentStateError, CartridgeRamPayloadKind,
};
use std::io;
use std::path::PathBuf;

#[test]
fn display_and_error_sources_are_human_readable() {
    assert_eq!(
        CartridgeSaveKeyError::Empty.to_string(),
        "save key must not be empty"
    );
    let exact_rom_stem_key =
        CartridgeSaveKey::new("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)")
            .expect("ordinary ROM filename punctuation should be valid");
    assert_eq!(
        exact_rom_stem_key.as_str(),
        "Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)"
    );
    assert_eq!(
        FilesystemCartridgeSaveBackend::new("saves").path_for_key(&exact_rom_stem_key),
        PathBuf::from("saves/Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gbsav")
    );
    let slot_extensions = [
        (CartridgeSaveFileExtension::P1, SAVE_FILE_EXTENSION),
        (CartridgeSaveFileExtension::P2, SAVE_FILE_EXTENSION_P2),
        (CartridgeSaveFileExtension::P3, SAVE_FILE_EXTENSION_P3),
        (CartridgeSaveFileExtension::P4, SAVE_FILE_EXTENSION_P4),
    ];
    for (file_extension, expected_suffix) in slot_extensions {
        let backend = FilesystemCartridgeSaveBackend::with_file_extension("saves", file_extension);
        assert_eq!(backend.file_extension(), file_extension);
        assert_eq!(
            backend.path_for_key(&exact_rom_stem_key),
            PathBuf::from(format!(
                "saves/Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).{expected_suffix}"
            ))
        );
    }
    assert_eq!(
        CartridgeSaveKeyError::InvalidCharacter {
            index: 3,
            character: '/',
        }
        .to_string(),
        "save key contains invalid character `/` at index 3"
    );

    let io_error = CartridgeSaveBackendError::Io {
        operation: "read save file",
        path: PathBuf::from("slot.gbsav"),
        source: io::Error::other("disk error"),
    };
    assert_eq!(io_error.to_string(), "read save file failed for slot.gbsav");
    assert!(std::error::Error::source(&io_error).is_some());

    assert_eq!(
        ExternalSaveError::UnsupportedPersistentState { state_kind: "Huc3" }.to_string(),
        "external .sav conversion does not support Huc3"
    );
    assert_eq!(
        ExternalSaveError::UnsupportedPersistenceProfile {
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 512 },
            },
        }
        .to_string(),
        "external .sav conversion does not support persistence profile PersistentRamAndRtc { ram: Mbc2Nibbles { cell_count: 512 } }"
    );
    assert_eq!(
        ExternalSaveError::StateProfileMismatch {
            state_kind: "Mbc2Ram",
            profile: CartridgePersistenceProfile::PersistentRtc,
        }
        .to_string(),
        "persistent state Mbc2Ram does not match cartridge persistence profile PersistentRtc"
    );
    assert_eq!(
        ExternalSaveError::InvalidLength {
            context: "linear RAM",
            expected: ExternalSaveLengthExpectation::Exact(8),
            actual: 4,
        }
        .to_string(),
        "invalid external .sav length for linear RAM: expected 8 bytes, got 4"
    );
    assert_eq!(
        ExternalSaveError::InvalidLength {
            context: "MBC2 RAM",
            expected: ExternalSaveLengthExpectation::Either {
                first: 256,
                second: 512,
            },
            actual: 257,
        }
        .to_string(),
        "invalid external .sav length for MBC2 RAM: expected 256 or 512 bytes, got 257"
    );

    let other_backend_errors = [
        (
            CartridgeSaveBackendError::InvalidMagic {
                actual: *b"BADSAVE!",
            },
            "invalid save magic",
        ),
        (
            CartridgeSaveBackendError::UnexpectedEof {
                offset: 3,
                needed: 4,
                remaining: 1,
            },
            "unexpected end of save payload at offset 3: needed 4 bytes but only 1 remain",
        ),
        (
            CartridgeSaveBackendError::UnsupportedFormatVersion { version: 7 },
            "unsupported save format version 7",
        ),
        (
            CartridgeSaveBackendError::UnsupportedRamPayloadKindTag { tag: 0xAA },
            "unsupported RAM payload kind tag 0xAA",
        ),
        (
            CartridgeSaveBackendError::UnsupportedPersistenceProfileTag { tag: 0xBB },
            "unsupported persistence profile tag 0xBB",
        ),
        (
            CartridgeSaveBackendError::UnsupportedPersistentStateTag { tag: 0xCC },
            "unsupported persistent state tag 0xCC",
        ),
        (
            CartridgeSaveBackendError::UnsupportedMachineSaveStateTag {
                field: "console_model",
                tag: 0xDD,
            },
            "unsupported machine save-state tag for console_model: 0xDD",
        ),
        (
            CartridgeSaveBackendError::InvalidBooleanTag {
                field: "rtc.halt",
                value: 2,
            },
            "invalid boolean tag for rtc.halt: 0x02",
        ),
        (
            CartridgeSaveBackendError::LengthOverflow {
                field: "MBC2 RAM cell_count",
                value: 1usize << 40,
            },
            "MBC2 RAM cell_count length 1099511627776 exceeds format capacity",
        ),
        (
            CartridgeSaveBackendError::InvalidMbc2NibbleValue {
                index: 7,
                value: 0x1F,
            },
            "invalid MBC2 nibble value 0x1F at logical cell 7",
        ),
        (
            CartridgeSaveBackendError::MachineSaveStateCodec {
                operation: "decode",
                message: "bad payload".to_string(),
            },
            "machine save-state decode failed: bad payload",
        ),
        (
            CartridgeSaveBackendError::MachineSaveStateMetadataMismatch,
            "machine save-state envelope metadata does not match payload metadata",
        ),
        (
            CartridgeSaveBackendError::TrailingBytes { remaining: 9 },
            "save payload has 9 trailing bytes",
        ),
    ];

    for (error, expected) in other_backend_errors {
        assert!(error.to_string().contains(expected));
        assert!(std::error::Error::source(&error).is_none());
    }

    let backend_error = HardwarePersistenceError::Backend(CartridgeSaveBackendError::Io {
        operation: "delete save file",
        path: PathBuf::from("slot.gbsav"),
        source: io::Error::other("permission denied"),
    });
    assert_eq!(
        backend_error.to_string(),
        "delete save file failed for slot.gbsav"
    );
    assert!(std::error::Error::source(&backend_error).is_some());

    let restore_error =
        HardwarePersistenceError::Restore(CartridgePersistentStateError::KindMismatch {
            expected: "MBC1 RAM",
            actual: "MBC2 RAM",
        });
    assert_eq!(
        restore_error.to_string(),
        "cartridge restore failed: KindMismatch { expected: \"MBC1 RAM\", actual: \"MBC2 RAM\" }"
    );
    assert!(std::error::Error::source(&restore_error).is_none());
}
