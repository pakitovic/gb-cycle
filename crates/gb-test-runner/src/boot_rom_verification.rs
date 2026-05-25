use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use gb_core::{BootRomAssetKind, HardwareRevision};
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
    MissingRoot {
        asset: BootRomAssetKind,
        env_var: &'static str,
    },
    MissingFile {
        asset: BootRomAssetKind,
        path: PathBuf,
    },
    ReadFile {
        asset: BootRomAssetKind,
        path: PathBuf,
        message: String,
    },
    HashMismatch {
        asset: BootRomAssetKind,
        path: PathBuf,
        expected_sha256: &'static str,
        actual_sha256: String,
    },
    SizeMismatch {
        asset: BootRomAssetKind,
        path: PathBuf,
        expected_size: usize,
        actual_size: usize,
    },
}

impl fmt::Display for BootRomVerificationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRoot { asset, env_var } => write!(
                f,
                "boot ROM root is not configured for {:?}; set {}",
                asset, env_var
            ),
            Self::MissingFile { asset, path } => write!(
                f,
                "boot ROM asset {:?} is missing or unreadable at {}",
                asset,
                path.display()
            ),
            Self::ReadFile {
                asset,
                path,
                message,
            } => write!(
                f,
                "failed to read boot ROM asset {:?} at {}: {}",
                asset,
                path.display(),
                message
            ),
            Self::HashMismatch {
                asset,
                path,
                expected_sha256,
                actual_sha256,
            } => write!(
                f,
                "boot ROM asset {:?} at {} has unexpected sha256: expected {}, got {}",
                asset,
                path.display(),
                expected_sha256,
                actual_sha256
            ),
            Self::SizeMismatch {
                asset,
                path,
                expected_size,
                actual_size,
            } => write!(
                f,
                "boot ROM asset {:?} at {} has unexpected size: expected {} bytes, got {}",
                asset,
                path.display(),
                expected_size,
                actual_size
            ),
        }
    }
}

impl std::error::Error for BootRomVerificationIssue {}

pub fn verify_boot_rom_file(
    path: &Path,
    revision: HardwareRevision,
) -> Result<(), BootRomVerificationIssue> {
    verify_boot_rom_asset_file(path, BootRomAssetKind::from_revision(revision))
}

pub fn verify_boot_rom_asset_file(
    path: &Path,
    asset: BootRomAssetKind,
) -> Result<(), BootRomVerificationIssue> {
    let bytes = fs::read(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            BootRomVerificationIssue::MissingFile {
                asset,
                path: path.to_path_buf(),
            }
        } else {
            BootRomVerificationIssue::ReadFile {
                asset,
                path: path.to_path_buf(),
                message: source.to_string(),
            }
        }
    })?;

    let expected_size = expected_boot_rom_asset_size(asset);
    if bytes.len() != expected_size {
        return Err(BootRomVerificationIssue::SizeMismatch {
            asset,
            path: path.to_path_buf(),
            expected_size,
            actual_size: bytes.len(),
        });
    }

    let actual_sha256 = sha256_hex(&bytes);
    let expected_sha256 = expected_boot_rom_asset_sha256(asset);
    if actual_sha256 != expected_sha256 {
        return Err(BootRomVerificationIssue::HashMismatch {
            asset,
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
    revision: HardwareRevision,
) -> Result<(), BootRomVerificationIssue> {
    enforce_boot_rom_asset_verification(mode, path, BootRomAssetKind::from_revision(revision))
}

pub fn enforce_boot_rom_asset_verification(
    mode: BootRomVerificationMode,
    path: &Path,
    asset: BootRomAssetKind,
) -> Result<(), BootRomVerificationIssue> {
    match mode {
        BootRomVerificationMode::Off => Ok(()),
        BootRomVerificationMode::Warn => {
            if let Err(issue) = verify_boot_rom_asset_file(path, asset) {
                eprintln!("warning: {issue}");
            }
            Ok(())
        }
        BootRomVerificationMode::Strict => verify_boot_rom_asset_file(path, asset),
    }
}

pub fn expected_boot_rom_sha256(revision: HardwareRevision) -> &'static str {
    expected_boot_rom_asset_sha256(BootRomAssetKind::from_revision(revision))
}

pub fn expected_boot_rom_asset_sha256(asset: BootRomAssetKind) -> &'static str {
    asset.expected_sha256()
}

pub const fn expected_boot_rom_size(revision: HardwareRevision) -> usize {
    BootRomAssetKind::from_revision(revision).expected_size()
}

pub const fn expected_boot_rom_asset_size(asset: BootRomAssetKind) -> usize {
    asset.expected_size()
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

    use gb_core::{BootRomAssetKind, HardwareRevision};

    use super::{
        BootRomVerificationIssue, BootRomVerificationMode, enforce_boot_rom_asset_verification,
        enforce_boot_rom_verification, expected_boot_rom_asset_sha256,
        expected_boot_rom_asset_size, expected_boot_rom_sha256, expected_boot_rom_size,
        verify_boot_rom_asset_file, verify_boot_rom_file,
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
    fn expected_hashes_match_known_boot_rom_dumps() {
        assert_eq!(
            expected_boot_rom_sha256(HardwareRevision::DmgCpu),
            "26e71cf01e301e5dc40e987cd2ecbf6d0276245890ac829db2a25323da86818e"
        );
        assert_eq!(
            expected_boot_rom_sha256(HardwareRevision::DmgCpuC),
            "cf053eccb4ccafff9e67339d4e78e98dce7d1ed59be819d2a1ba2232c6fce1c7"
        );
        assert_eq!(
            expected_boot_rom_sha256(HardwareRevision::CpuMgb),
            "a8cb5f4f1f16f2573ed2ecd8daedb9c5d1dd2c30a481f9b179b5d725d95eafe2"
        );
        assert_eq!(
            expected_boot_rom_sha256(HardwareRevision::CpuCgb),
            "3a307a41689bee99a9a32ea021bf45136906c86b2e4f06c806738398e4f92e45"
        );
        assert_eq!(
            expected_boot_rom_sha256(HardwareRevision::CpuCgbC),
            "b4f2e416a35eef52cba161b159c7c8523a92594facb924b3ede0d722867c50c7"
        );
        assert_eq!(
            expected_boot_rom_sha256(HardwareRevision::CpuCgbE),
            "c56299bedd56debdbf36442238636bf5887a65c5173b33995682052353804da9"
        );
        assert_eq!(
            expected_boot_rom_asset_sha256(BootRomAssetKind::Sgb),
            "0e4ddff32fc9d1eeaae812a157dd246459b00c9e14f2f61751f661f32361e360"
        );
        assert_eq!(
            expected_boot_rom_asset_sha256(BootRomAssetKind::Sgb2),
            "fd243c4fb27008986316ce3df29e9cfbcdc0cd52704970555a8bb76edbec3988"
        );
    }

    #[test]
    fn expected_sizes_match_canonical_boot_rom_assets() {
        assert_eq!(expected_boot_rom_size(HardwareRevision::DmgCpu), 256);
        assert_eq!(expected_boot_rom_size(HardwareRevision::DmgCpuC), 256);
        assert_eq!(expected_boot_rom_size(HardwareRevision::CpuMgb), 256);
        assert_eq!(expected_boot_rom_size(HardwareRevision::CpuCgb), 2304);
        assert_eq!(expected_boot_rom_size(HardwareRevision::CpuCgbC), 2304);
        assert_eq!(expected_boot_rom_size(HardwareRevision::CpuCgbE), 2304);
        assert_eq!(expected_boot_rom_asset_size(BootRomAssetKind::Sgb), 256);
        assert_eq!(expected_boot_rom_asset_size(BootRomAssetKind::Sgb2), 256);
    }

    #[test]
    fn sgb_asset_verification_uses_sgb_boot_rom_identity() {
        let temp_dir = unique_temp_dir("sgb-hash-mismatch");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let path = temp_dir.join("sgb_boot.bin");
        fs::write(
            &path,
            vec![0xA5; expected_boot_rom_asset_size(BootRomAssetKind::Sgb)],
        )
        .expect("sgb boot rom should be writable");

        let error = verify_boot_rom_asset_file(&path, BootRomAssetKind::Sgb)
            .expect_err("synthetic SGB bytes should fail strict verification");
        assert!(matches!(
            error,
            BootRomVerificationIssue::HashMismatch {
                asset: BootRomAssetKind::Sgb,
                ..
            }
        ));

        enforce_boot_rom_asset_verification(
            BootRomVerificationMode::Off,
            &path,
            BootRomAssetKind::Sgb2,
        )
        .expect("off mode should allow SGB2 verification to be skipped");

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn verification_reports_hash_mismatch_for_unexpected_bytes() {
        let temp_dir = unique_temp_dir("hash-mismatch");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let path = temp_dir.join("dmg_boot.bin");
        fs::write(
            &path,
            vec![0xA5; expected_boot_rom_size(HardwareRevision::DmgCpuC)],
        )
        .expect("boot rom should be writable");

        let error = verify_boot_rom_file(&path, HardwareRevision::DmgCpuC)
            .expect_err("unexpected bytes should fail strict verification");
        assert!(matches!(
            error,
            BootRomVerificationIssue::HashMismatch { .. }
        ));

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn strict_verification_rejects_noncanonical_cgb_image_sizes_before_hashing() {
        let temp_dir = unique_temp_dir("size-mismatch");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let path = temp_dir.join("cgb_boot.bin");
        fs::write(&path, vec![0x00; 0x0800]).expect("compact boot rom should be writable");

        let error = verify_boot_rom_file(&path, HardwareRevision::CpuCgbC)
            .expect_err("strict verification should reject compact CGB images");
        assert!(matches!(
            error,
            BootRomVerificationIssue::SizeMismatch {
                expected_size: 2304,
                actual_size: 2048,
                ..
            }
        ));

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn verification_reports_missing_files_in_strict_mode() {
        let path = unique_temp_dir("missing").join("mgb_boot.bin");
        let error = enforce_boot_rom_verification(
            BootRomVerificationMode::Strict,
            &path,
            HardwareRevision::CpuMgb,
        )
        .expect_err("strict verification should reject missing boot roms");
        assert!(matches!(
            error,
            BootRomVerificationIssue::MissingFile { .. }
        ));
    }

    #[test]
    fn verification_can_be_disabled_explicitly() {
        let path = unique_temp_dir("off").join("dmg0_boot.bin");
        enforce_boot_rom_verification(
            BootRomVerificationMode::Off,
            &path,
            HardwareRevision::DmgCpu,
        )
        .expect("off mode should skip verification");
    }

    #[test]
    fn verification_modes_expose_stable_names() {
        assert_eq!(BootRomVerificationMode::Off.name(), "off");
        assert_eq!(BootRomVerificationMode::Warn.name(), "warn");
        assert_eq!(BootRomVerificationMode::Strict.name(), "strict");
    }

    #[test]
    fn verification_reports_read_errors_for_non_file_paths() {
        let temp_dir = unique_temp_dir("read-error");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

        let error = verify_boot_rom_file(&temp_dir, HardwareRevision::DmgCpu)
            .expect_err("directories should surface a read-file error");
        assert!(matches!(error, BootRomVerificationIssue::ReadFile { .. }));
        assert!(error.to_string().contains("failed to read boot ROM asset"));

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn warn_mode_logs_but_does_not_fail_on_invalid_assets() {
        let temp_dir = unique_temp_dir("warn-mode");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let path = temp_dir.join("dmg_boot.bin");
        fs::write(&path, b"wrong").expect("boot rom should be writable");

        enforce_boot_rom_verification(
            BootRomVerificationMode::Warn,
            &path,
            HardwareRevision::DmgCpuC,
        )
        .expect("warn mode should not fail on invalid boot roms");

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn issue_display_mentions_revision_expected_and_actual_hashes() {
        let mismatch = BootRomVerificationIssue::HashMismatch {
            asset: BootRomAssetKind::Mgb,
            path: PathBuf::from("/tmp/mgb_boot.bin"),
            expected_sha256: "expected",
            actual_sha256: "actual".to_string(),
        };
        let rendered = mismatch.to_string();
        assert!(rendered.contains("Mgb"));
        assert!(rendered.contains("expected"));
        assert!(rendered.contains("actual"));
    }
}
