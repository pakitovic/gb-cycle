use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use gb_core::BootRomKind;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootRomVerificationMode {
    Off,
    Warn,
    Strict,
}

impl BootRomVerificationMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Warn => "warn",
            Self::Strict => "strict",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootRomVerificationIssue {
    MissingFile {
        kind: BootRomKind,
        path: PathBuf,
    },
    ReadFile {
        kind: BootRomKind,
        path: PathBuf,
        message: String,
    },
    HashMismatch {
        kind: BootRomKind,
        path: PathBuf,
        expected_sha256: &'static str,
        actual_sha256: String,
    },
}

impl fmt::Display for BootRomVerificationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFile { kind, path } => write!(
                f,
                "boot ROM asset {:?} is missing or unreadable at {}",
                kind,
                path.display()
            ),
            Self::ReadFile {
                kind,
                path,
                message,
            } => write!(
                f,
                "failed to read boot ROM asset {:?} at {}: {}",
                kind,
                path.display(),
                message
            ),
            Self::HashMismatch {
                kind,
                path,
                expected_sha256,
                actual_sha256,
            } => write!(
                f,
                "boot ROM asset {:?} at {} has unexpected sha256: expected {}, got {}",
                kind,
                path.display(),
                expected_sha256,
                actual_sha256
            ),
        }
    }
}

impl std::error::Error for BootRomVerificationIssue {}

pub fn verify_boot_rom_file(
    path: &Path,
    kind: BootRomKind,
) -> Result<(), BootRomVerificationIssue> {
    let bytes = fs::read(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            BootRomVerificationIssue::MissingFile {
                kind,
                path: path.to_path_buf(),
            }
        } else {
            BootRomVerificationIssue::ReadFile {
                kind,
                path: path.to_path_buf(),
                message: source.to_string(),
            }
        }
    })?;

    let actual_sha256 = sha256_hex(&bytes);
    let expected_sha256 = expected_boot_rom_sha256(kind);
    if actual_sha256 != expected_sha256 {
        return Err(BootRomVerificationIssue::HashMismatch {
            kind,
            path: path.to_path_buf(),
            expected_sha256,
            actual_sha256,
        });
    }

    Ok(())
}

pub fn enforce_boot_rom_verification(
    mode: BootRomVerificationMode,
    path: &Path,
    kind: BootRomKind,
) -> Result<(), BootRomVerificationIssue> {
    match mode {
        BootRomVerificationMode::Off => Ok(()),
        BootRomVerificationMode::Warn => {
            if let Err(issue) = verify_boot_rom_file(path, kind) {
                eprintln!("warning: {issue}");
            }
            Ok(())
        }
        BootRomVerificationMode::Strict => verify_boot_rom_file(path, kind),
    }
}

pub fn expected_boot_rom_sha256(kind: BootRomKind) -> &'static str {
    match kind {
        BootRomKind::Dmg0 => "26e71cf01e301e5dc40e987cd2ecbf6d0276245890ac829db2a25323da86818e",
        BootRomKind::Dmg => "cf053eccb4ccafff9e67339d4e78e98dce7d1ed59be819d2a1ba2232c6fce1c7",
        BootRomKind::Mgb => "a8cb5f4f1f16f2573ed2ecd8daedb9c5d1dd2c30a481f9b179b5d725d95eafe2",
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use gb_core::BootRomKind;

    use super::{
        BootRomVerificationIssue, BootRomVerificationMode, enforce_boot_rom_verification,
        expected_boot_rom_sha256, verify_boot_rom_file,
    };

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-boot-rom-verification-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn expected_hashes_match_known_dmg_family_dumps() {
        assert_eq!(
            expected_boot_rom_sha256(BootRomKind::Dmg0),
            "26e71cf01e301e5dc40e987cd2ecbf6d0276245890ac829db2a25323da86818e"
        );
        assert_eq!(
            expected_boot_rom_sha256(BootRomKind::Dmg),
            "cf053eccb4ccafff9e67339d4e78e98dce7d1ed59be819d2a1ba2232c6fce1c7"
        );
        assert_eq!(
            expected_boot_rom_sha256(BootRomKind::Mgb),
            "a8cb5f4f1f16f2573ed2ecd8daedb9c5d1dd2c30a481f9b179b5d725d95eafe2"
        );
    }

    #[test]
    fn verification_reports_hash_mismatch_for_unexpected_bytes() {
        let temp_dir = unique_temp_dir("hash-mismatch");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let path = temp_dir.join("dmg_boot.bin");
        fs::write(&path, b"not-a-real-dmg-boot-rom").expect("boot rom should be writable");

        let error = verify_boot_rom_file(&path, BootRomKind::Dmg)
            .expect_err("unexpected bytes should fail strict verification");
        assert!(matches!(
            error,
            BootRomVerificationIssue::HashMismatch { .. }
        ));

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn verification_reports_missing_files_in_strict_mode() {
        let path = unique_temp_dir("missing").join("mgb_boot.bin");
        let error =
            enforce_boot_rom_verification(BootRomVerificationMode::Strict, &path, BootRomKind::Mgb)
                .expect_err("strict verification should reject missing boot roms");
        assert!(matches!(
            error,
            BootRomVerificationIssue::MissingFile { .. }
        ));
    }

    #[test]
    fn verification_can_be_disabled_explicitly() {
        let path = unique_temp_dir("off").join("dmg0_boot.bin");
        enforce_boot_rom_verification(BootRomVerificationMode::Off, &path, BootRomKind::Dmg0)
            .expect("off mode should skip verification");
    }
}
