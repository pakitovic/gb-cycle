use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::cli::writeln_checked;
use super::manifest::{Source, SourceFamily, SourceFile};

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
    checkout_source_into_temp(temp_root, source)?;
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

fn checkout_source_into_temp(temp_root: &Path, source: &Source) -> Result<(), String> {
    fs::create_dir_all(temp_root).map_err(|error| {
        format!(
            "failed to create temporary fetch directory {} for source {}: {error}",
            temp_root.display(),
            source.id
        )
    })?;
    run_git(temp_root, ["init"], source)?;
    run_git(
        temp_root,
        ["remote", "add", "origin", &source.git_url],
        source,
    )?;

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
        ["fetch", "--depth", "1", "origin", &source.git_rev],
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
    let actual_hash = sha256_hex(&bytes);
    if actual_hash != file.sha256 {
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
