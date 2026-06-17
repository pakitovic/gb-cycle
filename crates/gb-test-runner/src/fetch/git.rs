use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use zip::ZipArchive;

use super::cli::writeln_checked;
use super::manifest::{Source, SourceArchiveFormat, SourceFamily, SourceFile, SourceLocation};

static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(super) struct FetchedSource {
    pub(super) source: Source,
    pub(super) temp_root: PathBuf,
}

pub(super) fn fetch_sources_into_temps<W: Write>(
    sources: &[Source],
    output: &mut W,
) -> Result<Vec<FetchedSource>, String> {
    let mut fetched_sources = Vec::new();
    for source in sources {
        let temp_root = unique_temp_fetch_root(source);
        if let Err(error) = fetch_source_into_temp(&temp_root, source, output) {
            let current_cleanup = remove_path_if_present(&temp_root, "temporary fetch directory");
            let previous_cleanup = cleanup_fetched_sources(&fetched_sources);
            return match (current_cleanup, previous_cleanup) {
                (Ok(()), Ok(())) => Err(error),
                (Err(cleanup_error), Ok(())) | (Ok(()), Err(cleanup_error)) => {
                    Err(format!("{error}; additionally {cleanup_error}"))
                }
                (Err(cleanup_error), Err(previous_cleanup_error)) => Err(format!(
                    "{error}; additionally {cleanup_error}; additionally {previous_cleanup_error}"
                )),
            };
        }
        fetched_sources.push(FetchedSource {
            source: source.clone(),
            temp_root,
        });
    }
    Ok(fetched_sources)
}

fn fetch_source_into_temp<W: Write>(
    temp_root: &Path,
    source: &Source,
    output: &mut W,
) -> Result<(), String> {
    remove_path_if_present(temp_root, "stale temporary fetch directory")?;
    match source.location()? {
        SourceLocation::Git { git_url, git_rev } => {
            checkout_source_into_temp(temp_root, source, git_url, git_rev)?;
        }
        SourceLocation::Archive {
            archive_url,
            archive_sha256,
            archive_format,
        } => {
            fetch_archive_source_into_temp(
                temp_root,
                source,
                archive_url,
                archive_sha256,
                archive_format,
            )?;
        }
        SourceLocation::FileBase { file_base_url } => {
            fetch_file_base_source_into_temp(temp_root, source, file_base_url)?;
        }
    }
    verify_required_files(temp_root, source)?;
    writeln_checked(
        output,
        &format!(
            "fetched test ROM source {} into temporary workspace {}",
            source.id,
            temp_root.display()
        ),
    )?;
    Ok(())
}

fn checkout_source_into_temp(
    temp_root: &Path,
    source: &Source,
    git_url: &str,
    git_rev: &str,
) -> Result<(), String> {
    fs::create_dir_all(temp_root).map_err(|error| {
        format!(
            "failed to create temporary fetch directory {} for source {}: {error}",
            temp_root.display(),
            source.id
        )
    })?;
    run_git(temp_root, ["init"], source)?;
    run_git(temp_root, ["remote", "add", "origin", git_url], source)?;

    let sparse_checkout_paths = source_sparse_checkout_paths(source);
    if !sparse_checkout_paths.is_empty() {
        run_git(temp_root, ["sparse-checkout", "init", "--cone"], source)?;

        let mut command = Command::new("git");
        command.current_dir(temp_root);
        command.arg("sparse-checkout").arg("set");
        for path in &sparse_checkout_paths {
            command.arg(path);
        }
        run_git_command(command, source, "sparse-checkout set")?;
    }

    run_git(
        temp_root,
        ["fetch", "--depth", "1", "origin", git_rev],
        source,
    )?;
    run_git(temp_root, ["checkout", "--detach", "FETCH_HEAD"], source)?;

    let git_dir = temp_root.join(".git");
    if git_dir.exists() {
        fs::remove_dir_all(&git_dir).map_err(|error| {
            format!(
                "failed to remove git metadata from source {} at {}: {error}",
                source.id,
                git_dir.display()
            )
        })?;
    }

    Ok(())
}

fn fetch_file_base_source_into_temp(
    temp_root: &Path,
    source: &Source,
    file_base_url: &str,
) -> Result<(), String> {
    fs::create_dir_all(temp_root).map_err(|error| {
        format!(
            "failed to create temporary fetch directory {} for source {}: {error}",
            temp_root.display(),
            source.id
        )
    })?;
    for family in &source.families {
        for file in &family.files {
            fetch_file_base_source_file(temp_root, source, family, file_base_url, file)?;
        }
    }
    Ok(())
}

fn fetch_file_base_source_file(
    temp_root: &Path,
    source: &Source,
    family: &SourceFamily,
    file_base_url: &str,
    file: &SourceFile,
) -> Result<(), String> {
    let url = source_file_url(file_base_url, &file.path);
    let target = temp_root.join(&file.path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create downloaded file parent {} for source {} family {}: {error}",
                parent.display(),
                source.id,
                family.id
            )
        })?;
    }
    download_url_to_path(&url, &target, source, "file")?;
    Ok(())
}

fn fetch_archive_source_into_temp(
    temp_root: &Path,
    source: &Source,
    archive_url: &str,
    archive_sha256: &str,
    archive_format: SourceArchiveFormat,
) -> Result<(), String> {
    fs::create_dir_all(temp_root).map_err(|error| {
        format!(
            "failed to create temporary fetch directory {} for source {}: {error}",
            temp_root.display(),
            source.id
        )
    })?;
    let archive_path = temp_root.join("source-archive");
    download_url_to_path(archive_url, &archive_path, source, "archive")?;
    let archive_bytes = fs::read(&archive_path).map_err(|error| {
        format!(
            "failed to read downloaded archive {} for source {}: {error}",
            archive_path.display(),
            source.id
        )
    })?;
    let actual_hash = sha256_hex(&archive_bytes);
    if !sha256_hex_eq(archive_sha256, &actual_hash) {
        return Err(format!(
            "archive hash mismatch for source {}: expected {}, got {}",
            source.id, archive_sha256, actual_hash
        ));
    }
    match archive_format {
        SourceArchiveFormat::Zip => extract_required_zip_files(temp_root, source, archive_bytes),
    }
}

fn download_url_to_path(
    url: &str,
    path: &Path,
    source: &Source,
    label: &str,
) -> Result<(), String> {
    if let Some(file_path) = url.strip_prefix("file://") {
        fs::copy(file_path, path).map_err(|error| {
            format!(
                "failed to copy {label} {} -> {} for source {}: {error}",
                url,
                path.display(),
                source.id
            )
        })?;
        return Ok(());
    }

    let mut command = Command::new("curl");
    command
        .arg("--fail")
        .arg("--location")
        .arg("--silent")
        .arg("--show-error")
        .arg("--output")
        .arg(path)
        .arg(url);
    let output = command.output().map_err(|error| {
        format!(
            "failed to spawn curl for {label} download in source {}: {error}",
            source.id
        )
    })?;
    if output.status.success() {
        return Ok(());
    }
    let mut message = format!(
        "curl {label} download failed for source {} with status {}",
        source.id, output.status
    );
    append_command_output(&mut message, "stdout", &output.stdout);
    append_command_output(&mut message, "stderr", &output.stderr);
    Err(message)
}

fn source_file_url(file_base_url: &str, path: &Path) -> String {
    format!(
        "{}/{}",
        file_base_url.trim_end_matches('/'),
        source_path_key(path)
    )
}

fn extract_required_zip_files(
    temp_root: &Path,
    source: &Source,
    archive_bytes: Vec<u8>,
) -> Result<(), String> {
    let mut archive = ZipArchive::new(Cursor::new(archive_bytes)).map_err(|error| {
        format!(
            "failed to open zip archive for source {}: {error}",
            source.id
        )
    })?;
    for family in &source.families {
        for file in &family.files {
            extract_required_zip_file(temp_root, source, family, file, &mut archive)?;
        }
    }
    Ok(())
}

fn extract_required_zip_file<R: io::Read + io::Seek>(
    temp_root: &Path,
    source: &Source,
    family: &SourceFamily,
    file: &SourceFile,
    archive: &mut ZipArchive<R>,
) -> Result<(), String> {
    let entry_name = source_path_key(&file.path);
    let mut entry = archive.by_name(&entry_name).map_err(|error| {
        format!(
            "failed to read required zip entry {} for source {} family {}: {error}",
            entry_name, source.id, family.id
        )
    })?;
    if entry.is_dir() {
        return Err(format!(
            "required zip entry {} for source {} family {} is a directory",
            entry_name, source.id, family.id
        ));
    }
    let target = temp_root.join(&file.path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create extracted archive parent {} for source {} family {}: {error}",
                parent.display(),
                source.id,
                family.id
            )
        })?;
    }
    let mut output = fs::File::create(&target).map_err(|error| {
        format!(
            "failed to create extracted archive file {} for source {} family {}: {error}",
            target.display(),
            source.id,
            family.id
        )
    })?;
    io::copy(&mut entry, &mut output).map_err(|error| {
        format!(
            "failed to extract zip entry {} -> {} for source {} family {}: {error}",
            entry_name,
            target.display(),
            source.id,
            family.id
        )
    })?;
    Ok(())
}

pub(super) fn source_sparse_checkout_paths(source: &Source) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for family in &source.families {
        for sparse_path in &family.sparse_paths {
            unique.insert(sparse_path.to_string_lossy().to_string());
        }
    }
    unique.into_iter().collect()
}

fn verify_required_files(root: &Path, source: &Source) -> Result<(), String> {
    for family in &source.families {
        for file in &family.files {
            verify_required_file(root, source, family, file)?;
        }
    }
    Ok(())
}

fn verify_required_file(
    root: &Path,
    source: &Source,
    family: &SourceFamily,
    file: &SourceFile,
) -> Result<(), String> {
    let path = root.join(&file.path);
    let bytes = fs::read(&path).map_err(|error| {
        format!(
            "failed to read required file {} for source {} family {}: {error}",
            path.display(),
            source.id,
            family.id
        )
    })?;
    if let Some(expected_size) = file.size {
        let actual_size = u64::try_from(bytes.len()).expect("usize should fit into u64");
        if actual_size != expected_size {
            return Err(format!(
                "size mismatch for source {} family {} file {}: expected {} bytes, got {}",
                source.id,
                family.id,
                file.path.display(),
                expected_size,
                actual_size
            ));
        }
    }
    let actual_hash = sha256_hex(&bytes);
    if !sha256_hex_eq(&file.sha256, &actual_hash) {
        return Err(format!(
            "hash mismatch for source {} family {} file {}: expected {}, got {}",
            source.id,
            family.id,
            file.path.display(),
            file.sha256,
            actual_hash
        ));
    }
    Ok(())
}

fn source_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn cleanup_fetched_sources(fetched_sources: &[FetchedSource]) -> Result<(), String> {
    for fetched_source in fetched_sources {
        remove_path_if_present(&fetched_source.temp_root, "temporary fetch directory")?;
    }
    Ok(())
}

fn unique_temp_fetch_root(source: &Source) -> PathBuf {
    let source_name = source.id.replace('/', "-");
    unique_temp_path(&format!("test-rom-fetch-{source_name}"))
}

pub(super) fn unique_temp_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let sequence = TEMP_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!(
        "gb-cycle-{label}-{}-{nanos}-{sequence}",
        process::id()
    ))
}

pub(super) fn remove_path_if_present(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)
            .map_err(|error| format!("failed to remove {label} {}: {error}", path.display())),
        Ok(_) => fs::remove_file(path)
            .map_err(|error| format!("failed to remove {label} {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to inspect {label} {}: {error}",
            path.display()
        )),
    }
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

pub(super) fn sha256_hex_eq(expected: &str, actual: &str) -> bool {
    expected.eq_ignore_ascii_case(actual)
}

pub(super) fn scrub_inherited_git_repository_context(command: &mut Command) {
    for key in [
        "GIT_COMMON_DIR",
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_NAMESPACE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_PREFIX",
        "GIT_WORK_TREE",
    ] {
        command.env_remove(key);
    }
}

fn run_git<const N: usize>(
    current_dir: &Path,
    args: [&str; N],
    source: &Source,
) -> Result<(), String> {
    let mut command = Command::new("git");
    command.current_dir(current_dir);
    command.args(args);
    run_git_command(command, source, "git command")
}

fn run_git_command(mut command: Command, source: &Source, label: &str) -> Result<(), String> {
    scrub_inherited_git_repository_context(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("failed to spawn {label} for source {}: {error}", source.id))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "{label} failed for source {} with status {}: {}",
        source.id,
        output
            .status
            .code()
            .map_or_else(|| "signal".to_string(), |code| code.to_string()),
        stderr.trim()
    ))
}

fn append_command_output(message: &mut String, label: &str, output: &[u8]) {
    let text = String::from_utf8_lossy(output);
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        let _ = write!(message, "; {label}: {trimmed}");
    }
}
