use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub(crate) fn clean_report_runtime_dirs(
    workspace_root: &Path,
    store_dir: &Path,
    status_dir: &Path,
    artifact_dir: &Path,
) -> Result<(), String> {
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
