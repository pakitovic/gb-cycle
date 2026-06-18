use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path};

pub(crate) fn clean_suite_runtime_dirs<'a>(
    workspace_root: &Path,
    store_dir: &Path,
    status_dir: &Path,
    artifact_dir: &Path,
    suite_names: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    validate_runtime_path(store_dir, "report store_dir", true)?;
    validate_runtime_path(status_dir, "report status_dir", false)?;
    validate_runtime_path(artifact_dir, "report artifact_dir", false)?;

    let store_root = workspace_root.join("test").join(store_dir);
    let status_root = store_root.join(status_dir);
    let artifact_root = store_root.join(artifact_dir);
    for suite_name in suite_names {
        validate_runtime_leaf(suite_name, "suite name")?;
        remove_runtime_file(
            &status_root.join(format!("{suite_name}.json")),
            "test ROM suite status file",
        )?;
        remove_runtime_dir(
            &artifact_root.join(suite_name),
            "test ROM suite artifact directory",
        )?;
    }
    Ok(())
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

fn validate_runtime_leaf(value: &str, field: &str) -> Result<(), String> {
    let mut components = Path::new(value).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(format!("{field} {value:?} must be a relative path leaf")),
    }
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

fn remove_runtime_file(path: &Path, label: &str) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove {label} {}: {error}",
            path.display()
        )),
    }
}
