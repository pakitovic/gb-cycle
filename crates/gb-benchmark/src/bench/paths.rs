use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(super) const BENCH_OUTPUT_DIR: &str = "test/bench";

pub(super) fn case_files(case_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut cases = fs::read_dir(case_dir)
        .map_err(|error| {
            format!(
                "failed to read case directory {}: {error}",
                case_dir.display()
            )
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read case directory {}: {error}",
                case_dir.display()
            )
        })?;
    cases.retain(|path| path.is_file() && path.extension().and_then(OsStr::to_str) == Some("toml"));
    cases.sort_by(|left, right| {
        left.file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .cmp(
                &right
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
            )
    });
    Ok(cases)
}

pub(super) fn rom_files(rom_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut roms = Vec::new();
    collect_rom_files(rom_dir, &mut roms)?;
    roms.sort_by(|left, right| {
        left.display()
            .to_string()
            .to_ascii_lowercase()
            .cmp(&right.display().to_string().to_ascii_lowercase())
    });
    Ok(roms)
}

fn collect_rom_files(dir: &Path, roms: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir)
        .map_err(|error| format!("failed to read ROM directory {}: {error}", dir.display()))?
    {
        let path = entry
            .map_err(|error| format!("failed to read ROM directory {}: {error}", dir.display()))?
            .path();
        if path.is_dir() {
            collect_rom_files(&path, roms)?;
        } else if path.is_file() && is_rom_path(&path) {
            roms.push(path);
        }
    }
    Ok(())
}

fn is_rom_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("gb" | "gbc")
    )
}

pub(super) fn resolve_existing_dir(
    path: &Path,
    current_dir: &Path,
    label: &str,
) -> Result<PathBuf, String> {
    let path = absolutize(path, current_dir);
    if !path.is_dir() {
        return Err(format!("{label} is not a directory: {}", path.display()));
    }
    canonicalize_lossy(&path)
}

pub(super) fn resolve_or_create_dir(
    path: &Path,
    current_dir: &Path,
    label: &str,
) -> Result<PathBuf, String> {
    let path = absolutize(path, current_dir);
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create {label} {}: {error}", path.display()))?;
    resolve_existing_dir(&path, current_dir, label)
}

pub(super) fn resolve_existing_file(
    path: &Path,
    current_dir: &Path,
    label: &str,
) -> Result<PathBuf, String> {
    let path = absolutize(path, current_dir);
    if !path.is_file() {
        return Err(format!("{label} not found: {}", path.display()));
    }
    canonicalize_lossy(&path)
}

fn absolutize(path: &Path, current_dir: &Path) -> PathBuf {
    let path = expand_tilde(path);
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let text = path.display().to_string();
    if text == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(rest) = text.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    path.to_path_buf()
}

pub(super) fn canonicalize_lossy(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("failed to resolve {}: {error}", path.display()))
}

pub(super) fn bench_output_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(BENCH_OUTPUT_DIR)
}

pub(super) fn workspace_binary(workspace_root: &Path, name: &str) -> PathBuf {
    workspace_root
        .join("target")
        .join("release-max")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

pub(super) fn default_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-benchmark")
        .to_path_buf()
}

pub(super) fn case_label(case_dir: &Path, case_path: &Path) -> String {
    relative_display(case_path, case_dir)
}

pub(super) fn relative_display(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn write_all<W>(output: &mut W, text: &str) -> Result<(), String>
where
    W: Write,
{
    output.write_all(text.as_bytes()).map_err(io_error)
}

pub(super) fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
