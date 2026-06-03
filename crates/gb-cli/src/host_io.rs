use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) fn resolve_path(current_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_dir.join(path)
    }
}

pub(crate) fn validate_explicit_directory_input(
    flag: &str,
    explicit_path: Option<&Path>,
    resolved_path: &Path,
) -> Result<(), String> {
    if explicit_path.is_some() {
        validate_directory_input(flag, resolved_path)?;
    }
    Ok(())
}

pub(crate) fn validate_directory_input(flag: &str, path: &Path) -> Result<(), String> {
    if path.exists() && !path.is_dir() {
        return Err(format!(
            "{flag} expects a directory path: {}",
            path.display()
        ));
    }
    Ok(())
}

pub(crate) fn write_bytes_with_parent(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create directory {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(crate) fn write_text_file_with_parent(path: &Path, text: &str) -> Result<(), String> {
    write_bytes_with_parent(path, text.as_bytes())
}

pub(crate) fn write_text(writer: &mut dyn Write, text: &str) -> Result<(), String> {
    if let Err(error) = writer.write_all(text.as_bytes()) {
        return Err(format!("failed to write output: {error}"));
    }
    Ok(())
}

pub(crate) fn writeln_checked(writer: &mut dyn Write, line: &str) -> Result<(), String> {
    if let Err(error) = writer.write_all(line.as_bytes()) {
        return Err(format!("failed to write output: {error}"));
    }
    if let Err(error) = writer.write_all(b"\n") {
        return Err(format!("failed to write output: {error}"));
    }
    Ok(())
}
