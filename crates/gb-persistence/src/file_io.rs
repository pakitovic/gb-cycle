use crate::backend::CartridgeSaveBackendError;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) fn write_save_file_with_safe_replace(
    path: &Path,
    bytes: &[u8],
) -> Result<(), CartridgeSaveBackendError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| CartridgeSaveBackendError::Io {
        operation: "create save directory",
        path: parent.to_path_buf(),
        source,
    })?;

    let temp_path = append_extension_suffix(path, ".tmp");
    let backup_path = append_extension_suffix(path, ".bak");
    if backup_path.exists() {
        fs::remove_file(&backup_path).map_err(|source| CartridgeSaveBackendError::Io {
            operation: "remove stale backup save file",
            path: backup_path.clone(),
            source,
        })?;
    }

    {
        let mut file =
            File::create(&temp_path).map_err(|source| CartridgeSaveBackendError::Io {
                operation: "create temporary save file",
                path: temp_path.clone(),
                source,
            })?;
        file.write_all(bytes)
            .map_err(|source| CartridgeSaveBackendError::Io {
                operation: "write temporary save file",
                path: temp_path.clone(),
                source,
            })?;
        file.sync_all()
            .map_err(|source| CartridgeSaveBackendError::Io {
                operation: "sync temporary save file",
                path: temp_path.clone(),
                source,
            })?;
    }

    let had_existing_target = path.exists();
    if had_existing_target {
        fs::rename(path, &backup_path).map_err(|source| CartridgeSaveBackendError::Io {
            operation: "move previous save file to backup",
            path: path.to_path_buf(),
            source,
        })?;
    }

    match fs::rename(&temp_path, path) {
        Ok(()) => {}
        Err(source) => {
            if had_existing_target && !path.exists() {
                let _ = fs::rename(&backup_path, path);
            }
            let _ = fs::remove_file(&temp_path);
            return Err(CartridgeSaveBackendError::Io {
                operation: "replace save file",
                path: path.to_path_buf(),
                source,
            });
        }
    }

    if had_existing_target && backup_path.exists() {
        fs::remove_file(&backup_path).map_err(|source| CartridgeSaveBackendError::Io {
            operation: "remove backup save file",
            path: backup_path,
            source,
        })?;
    }

    Ok(())
}

fn append_extension_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = OsString::from(path.as_os_str());
    os.push(suffix);
    PathBuf::from(os)
}
