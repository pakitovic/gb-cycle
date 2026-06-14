use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path};

pub(crate) fn clean_report_runtime_dirs(
    workspace_root: &Path,
    store_dir: &Path,
    status_dir: &Path,
    artifact_dir: &Path,
) -> Result<(), String> {
    validate_runtime_path(store_dir, "report store_dir", true)?;
    validate_runtime_path(status_dir, "report status_dir", false)?;
    validate_runtime_path(artifact_dir, "report artifact_dir", false)?;

    let store_root = workspace_root.join("test").join(store_dir);
    remove_runtime_dir(
        &store_root.join(status_dir),
        "test ROM report status directory",
    )?;
    remove_runtime_dir(
        &store_root.join(artifact_dir),
        "test ROM report artifact directory",
    )
}

fn validate_runtime_path(path: &Path, field: &str, allow_empty: bool) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        if allow_empty {
            return Ok(());
        }
        return Err(format!("{field} must not be empty"));
    }
    if path.is_absolute() {
        return Err(format!("{field} {} must be relative", path.display()));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::ParentDir => {
                return Err(format!(
                    "{field} {} must not contain parent components",
                    path.display()
                ));
            }
            Component::CurDir => {
                return Err(format!(
                    "{field} {} must not contain current-directory components",
                    path.display()
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("{field} {} must be relative", path.display()));
            }
        }
    }
    Ok(())
}

fn remove_runtime_dir(path: &Path, label: &str) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove {label} {}: {error}",
            path.display()
        )),
    }
}
