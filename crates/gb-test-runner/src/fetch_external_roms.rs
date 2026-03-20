use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{self, Command};

use sha2::{Digest, Sha256};

use crate::{
    ExternalRomRequiredFile, ExternalRomSource, ExternalRomSourceManifest, external_rom_store_root,
    load_external_rom_source_manifest,
};

const FETCH_METADATA_FILE_NAME: &str = ".gb-cycle-fetch.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
enum FetchExternalRomsAction {
    ShowHelp,
    Fetch(FetchExternalRomsOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FetchExternalRomsOptions {
    force: bool,
    requested_source_ids: Vec<String>,
}

pub fn fetch_external_roms_help_text() -> &'static str {
    concat!(
        "Usage: cargo run -p gb-test-runner --bin fetch_external_roms -- [--force] [source-id ...]\n",
        "\n",
        "Downloads repo-managed external ROM sources into .roms/external-test/.\n",
    )
}

pub fn run_fetch_external_roms_command<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_fetch_external_roms_arguments(arguments)? {
        FetchExternalRomsAction::ShowHelp => write_all(output, fetch_external_roms_help_text()),
        FetchExternalRomsAction::Fetch(options) => {
            let manifest = load_external_rom_source_manifest(workspace_root)
                .map_err(|error| error.to_string())?;
            let selected_sources = select_sources(&manifest, &options.requested_source_ids)?;
            let store_root = external_rom_store_root(workspace_root);
            fs::create_dir_all(&store_root).map_err(|error| {
                format!(
                    "failed to create external ROM store {}: {error}",
                    store_root.display()
                )
            })?;

            for source in &selected_sources {
                fetch_source(&store_root, source, options.force, output)?;
            }

            Ok(())
        }
    }
}

fn parse_fetch_external_roms_arguments<I, S>(
    arguments: I,
) -> Result<FetchExternalRomsAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut force = false;
    let mut requested_source_ids = Vec::new();

    for argument in arguments {
        match argument.as_ref() {
            "--force" => force = true,
            "--help" | "-h" => return Ok(FetchExternalRomsAction::ShowHelp),
            other => requested_source_ids.push(other.to_string()),
        }
    }

    Ok(FetchExternalRomsAction::Fetch(FetchExternalRomsOptions {
        force,
        requested_source_ids,
    }))
}

fn select_sources(
    manifest: &ExternalRomSourceManifest,
    requested_source_ids: &[String],
) -> Result<Vec<ExternalRomSource>, String> {
    if requested_source_ids.is_empty() {
        return Ok(manifest.sources().to_vec());
    }

    let mut selected = Vec::with_capacity(requested_source_ids.len());
    for source_id in requested_source_ids {
        let Some(source) = manifest.source_by_id(source_id) else {
            return Err(format!(
                "unknown external ROM source id {source_id:?}; run without arguments to fetch all configured sources"
            ));
        };
        selected.push(source.clone());
    }

    Ok(selected)
}

fn fetch_source<W: Write>(
    store_root: &Path,
    source: &ExternalRomSource,
    force: bool,
    output: &mut W,
) -> Result<(), String> {
    let final_root = store_root.join(&source.local_dir);

    if !force
        && source_is_current(&final_root, source)?
        && verify_required_files(&final_root, source).is_ok()
    {
        return writeln_checked(
            output,
            &format!(
                "external ROM source {} already available at {}",
                source.id,
                final_root.display()
            ),
        );
    }

    let temp_root = store_root.join(format!(".tmp-{}-{}", source.local_dir, process::id()));
    if temp_root.exists() {
        fs::remove_dir_all(&temp_root).map_err(|error| {
            format!(
                "failed to remove stale temporary directory {}: {error}",
                temp_root.display()
            )
        })?;
    }

    checkout_source_into_temp(&temp_root, source)?;
    verify_required_files(&temp_root, source)?;
    write_fetch_metadata(&temp_root, source)?;

    if final_root.exists() {
        fs::remove_dir_all(&final_root).map_err(|error| {
            format!(
                "failed to replace previous external ROM source {} at {}: {error}",
                source.id,
                final_root.display()
            )
        })?;
    }

    fs::rename(&temp_root, &final_root).map_err(|error| {
        format!(
            "failed to move fetched source {} into {}: {error}",
            source.id,
            final_root.display()
        )
    })?;

    writeln_checked(
        output,
        &format!(
            "fetched external ROM source {} into {}",
            source.id,
            final_root.display()
        ),
    )
}

fn source_is_current(root: &Path, source: &ExternalRomSource) -> Result<bool, String> {
    let metadata_path = root.join(FETCH_METADATA_FILE_NAME);
    let metadata = match fs::read_to_string(&metadata_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to read fetch metadata {}: {error}",
                metadata_path.display()
            ));
        }
    };

    Ok(metadata == render_fetch_metadata(source))
}

fn checkout_source_into_temp(temp_root: &Path, source: &ExternalRomSource) -> Result<(), String> {
    run_git(
        None,
        [
            "init",
            temp_root.to_str().expect("temp path should be utf-8"),
        ],
        source,
    )?;
    run_git(
        Some(temp_root),
        ["remote", "add", "origin", &source.git_url],
        source,
    )?;

    let top_level_paths = source_top_level_paths(source);
    if !top_level_paths.is_empty() {
        run_git(
            Some(temp_root),
            ["sparse-checkout", "init", "--cone"],
            source,
        )?;

        let mut command = Command::new("git");
        command.current_dir(temp_root);
        command.arg("sparse-checkout").arg("set");
        for path in &top_level_paths {
            command.arg(path);
        }
        run_git_command(command, source, "sparse-checkout set")?;
    }

    run_git(
        Some(temp_root),
        ["fetch", "--depth", "1", "origin", &source.git_rev],
        source,
    )?;
    run_git(
        Some(temp_root),
        ["checkout", "--detach", "FETCH_HEAD"],
        source,
    )?;

    let git_dir = temp_root.join(".git");
    if git_dir.exists() {
        fs::remove_dir_all(&git_dir).map_err(|error| {
            format!(
                "failed to remove git metadata from fetched source {} at {}: {error}",
                source.id,
                git_dir.display()
            )
        })?;
    }

    Ok(())
}

fn source_top_level_paths(source: &ExternalRomSource) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for required_file in &source.required_files {
        if let Some(component) = required_file.path.components().next() {
            unique.insert(component.as_os_str().to_string_lossy().into_owned());
        }
    }
    unique.into_iter().collect()
}

fn verify_required_files(root: &Path, source: &ExternalRomSource) -> Result<(), String> {
    for required_file in &source.required_files {
        verify_required_file(root, source, required_file)?;
    }

    Ok(())
}

fn verify_required_file(
    root: &Path,
    source: &ExternalRomSource,
    required_file: &ExternalRomRequiredFile,
) -> Result<(), String> {
    let path = root.join(&required_file.path);
    let bytes = fs::read(&path).map_err(|error| {
        format!(
            "failed to read required file {} for source {}: {error}",
            path.display(),
            source.id
        )
    })?;
    let actual_hash = sha256_hex(&bytes);
    if actual_hash != required_file.sha256 {
        return Err(format!(
            "hash mismatch for source {} file {}: expected {}, got {}",
            source.id,
            required_file.path.display(),
            required_file.sha256,
            actual_hash
        ));
    }

    Ok(())
}

fn write_fetch_metadata(root: &Path, source: &ExternalRomSource) -> Result<(), String> {
    let path = root.join(FETCH_METADATA_FILE_NAME);
    fs::write(&path, render_fetch_metadata(source))
        .map_err(|error| format!("failed to write fetch metadata {}: {error}", path.display()))
}

fn render_fetch_metadata(source: &ExternalRomSource) -> String {
    format!(
        concat!(
            "version = 1\n",
            "source_id = \"{}\"\n",
            "git_url = \"{}\"\n",
            "git_rev = \"{}\"\n",
        ),
        source.id, source.git_url, source.git_rev
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn run_git<const N: usize>(
    current_dir: Option<&Path>,
    args: [&str; N],
    source: &ExternalRomSource,
) -> Result<(), String> {
    let mut command = Command::new("git");
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    command.args(args);
    run_git_command(command, source, "git command")
}

fn run_git_command(
    mut command: Command,
    source: &ExternalRomSource,
    label: &str,
) -> Result<(), String> {
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

fn write_all<W: Write>(output: &mut W, text: &str) -> Result<(), String> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write command output: {error}"))
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}").map_err(|error| format!("failed to write command output: {error}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{EXTERNAL_ROM_SOURCE_MANIFEST_PATH, external_rom_store_root};

    use super::{
        FETCH_METADATA_FILE_NAME, FetchExternalRomsAction, FetchExternalRomsOptions,
        fetch_external_roms_help_text, fetch_source, parse_fetch_external_roms_arguments,
        render_fetch_metadata, run_fetch_external_roms_command, select_sources, sha256_hex,
        source_is_current, source_top_level_paths, verify_required_files, write_fetch_metadata,
    };
    use crate::{ExternalRomRequiredFile, ExternalRomSource, load_external_rom_source_manifest};

    fn unique_temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gb-cycle-fetch-external-roms-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn write_manifest(workspace_root: &Path, source: &ExternalRomSource) {
        let manifest_path = workspace_root.join(EXTERNAL_ROM_SOURCE_MANIFEST_PATH);
        let manifest_parent = manifest_path
            .parent()
            .expect("manifest path should have a parent");
        fs::create_dir_all(manifest_parent).expect("manifest parent should be creatable");

        fs::write(
            manifest_path,
            format!(
                concat!(
                    "version = 1\n\n",
                    "[[source]]\n",
                    "id = {:?}\n",
                    "git_url = {:?}\n",
                    "git_rev = {:?}\n",
                    "local_dir = {:?}\n",
                    "root_env_var = {:?}\n\n",
                    "[[source.required_file]]\n",
                    "path = {:?}\n",
                    "sha256 = {:?}\n",
                ),
                source.id,
                source.git_url,
                source.git_rev,
                source.local_dir,
                source.root_env_var,
                source.required_files[0].path.display().to_string(),
                source.required_files[0].sha256,
            ),
        )
        .expect("manifest should be writable");
    }

    fn git(args: &[&str], current_dir: &Path) {
        let output = Command::new("git")
            .current_dir(current_dir)
            .args(args)
            .output()
            .expect("git command should spawn");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn create_upstream_repo(root: &Path, required_file_path: &Path, contents: &[u8]) -> String {
        fs::create_dir_all(
            root.join(
                required_file_path
                    .parent()
                    .expect("required file path should have a parent"),
            ),
        )
        .expect("upstream subdir should be creatable");
        fs::write(root.join(required_file_path), contents)
            .expect("required file should be writable");

        git(&["init"], root);
        git(&["config", "user.email", "gb-cycle@example.invalid"], root);
        git(&["config", "user.name", "gb-cycle tests"], root);
        git(&["add", "."], root);
        git(&["commit", "-m", "fixture"], root);

        let output = Command::new("git")
            .current_dir(root)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git rev-parse should spawn");
        assert!(output.status.success(), "git rev-parse should succeed");
        String::from_utf8(output.stdout)
            .expect("git hash should be utf-8")
            .trim()
            .to_string()
    }

    fn build_source(git_url: String, git_rev: String, sha256: String) -> ExternalRomSource {
        ExternalRomSource {
            id: "retrio-gb-test-roms".to_string(),
            git_url,
            git_rev,
            local_dir: "retrio-gb-test-roms".to_string(),
            root_env_var: "GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT".to_string(),
            required_files: vec![ExternalRomRequiredFile {
                path: PathBuf::from("cpu_instrs/individual/01-special.gb"),
                sha256,
            }],
        }
    }

    #[test]
    fn parse_fetch_command_arguments_supports_help_force_and_source_ids() {
        assert_eq!(
            parse_fetch_external_roms_arguments(["--help"]).expect("help should parse"),
            FetchExternalRomsAction::ShowHelp
        );
        assert_eq!(
            parse_fetch_external_roms_arguments(["--force", "retrio-gb-test-roms"])
                .expect("fetch args should parse"),
            FetchExternalRomsAction::Fetch(FetchExternalRomsOptions {
                force: true,
                requested_source_ids: vec!["retrio-gb-test-roms".to_string()],
            })
        );
    }

    #[test]
    fn select_sources_rejects_unknown_ids() {
        let source = build_source(
            "https://example.invalid/retrio.git".to_string(),
            "deadbeef".to_string(),
            "00".repeat(32),
        );
        let workspace_root = unique_temp_dir("select-sources");
        write_manifest(&workspace_root, &source);
        let manifest =
            load_external_rom_source_manifest(&workspace_root).expect("manifest should load");

        let error = select_sources(&manifest, &["unknown".to_string()])
            .expect_err("unknown source id should be rejected");

        assert!(error.contains("unknown external ROM source id"));
        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn source_top_level_paths_keep_sparse_checkout_inputs_unique() {
        let source = ExternalRomSource {
            id: "source".to_string(),
            git_url: "https://example.invalid/retrio.git".to_string(),
            git_rev: "deadbeef".to_string(),
            local_dir: "retrio".to_string(),
            root_env_var: "GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT".to_string(),
            required_files: vec![
                ExternalRomRequiredFile {
                    path: PathBuf::from("cpu_instrs/individual/01-special.gb"),
                    sha256: "a".repeat(64),
                },
                ExternalRomRequiredFile {
                    path: PathBuf::from("cpu_instrs/individual/02-interrupts.gb"),
                    sha256: "b".repeat(64),
                },
                ExternalRomRequiredFile {
                    path: PathBuf::from("mem_timing/mem_timing.gb"),
                    sha256: "c".repeat(64),
                },
            ],
        };

        assert_eq!(
            source_top_level_paths(&source),
            vec!["cpu_instrs".to_string(), "mem_timing".to_string()]
        );
    }

    #[test]
    fn fetch_source_clones_pinned_files_and_writes_metadata() {
        let workspace_root = unique_temp_dir("clone");
        let upstream_root = workspace_root.join("upstream");
        fs::create_dir_all(&upstream_root).expect("upstream root should be creatable");

        let payload = b"official-rom".to_vec();
        let required_file_path = PathBuf::from("cpu_instrs/individual/01-special.gb");
        let git_rev = create_upstream_repo(&upstream_root, &required_file_path, &payload);
        let source = build_source(
            upstream_root.display().to_string(),
            git_rev,
            sha256_hex(&payload),
        );
        write_manifest(&workspace_root, &source);

        let store_root = external_rom_store_root(&workspace_root);
        fs::create_dir_all(&store_root).expect("store root should be creatable");
        let mut output = Vec::new();

        fetch_source(&store_root, &source, false, &mut output)
            .expect("fetch source should succeed");

        let final_root = store_root.join(&source.local_dir);
        assert_eq!(
            fs::read(final_root.join(&required_file_path))
                .expect("fetched file should be readable"),
            payload
        );
        assert_eq!(
            fs::read_to_string(final_root.join(FETCH_METADATA_FILE_NAME))
                .expect("fetch metadata should be readable"),
            render_fetch_metadata(&source)
        );
        assert!(!final_root.join(".git").exists());
        assert!(
            String::from_utf8(output)
                .expect("command output should be utf-8")
                .contains("fetched external ROM source retrio-gb-test-roms")
        );

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn fetch_command_reuses_a_current_source_without_running_git() {
        let workspace_root = unique_temp_dir("current");
        let store_root = external_rom_store_root(&workspace_root);
        let payload = b"already-present".to_vec();
        let source = build_source(
            "https://example.invalid/retrio.git".to_string(),
            "deadbeef".to_string(),
            sha256_hex(&payload),
        );
        write_manifest(&workspace_root, &source);

        let final_root = store_root.join(&source.local_dir);
        fs::create_dir_all(
            final_root.join(
                source.required_files[0]
                    .path
                    .parent()
                    .expect("required file path should have a parent"),
            ),
        )
        .expect("final root should be creatable");
        fs::write(final_root.join(&source.required_files[0].path), &payload)
            .expect("required file should be writable");
        write_fetch_metadata(&final_root, &source).expect("fetch metadata should be writable");

        let mut output = Vec::new();
        run_fetch_external_roms_command(["retrio-gb-test-roms"], &workspace_root, &mut output)
            .expect("fetch command should succeed");

        assert!(source_is_current(&final_root, &source).expect("current source check should work"));
        verify_required_files(&final_root, &source)
            .expect("required file verification should succeed");
        assert!(
            String::from_utf8(output)
                .expect("command output should be utf-8")
                .contains("already available")
        );

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn fetch_command_help_does_not_require_a_manifest() {
        let workspace_root = unique_temp_dir("help");
        let mut output = Vec::new();

        run_fetch_external_roms_command(["--help"], &workspace_root, &mut output)
            .expect("help command should succeed");

        assert_eq!(
            String::from_utf8(output).expect("command output should be utf-8"),
            fetch_external_roms_help_text()
        );
    }

    #[test]
    fn verify_required_files_reports_hash_mismatches() {
        let root = unique_temp_dir("hash-mismatch");
        let source = build_source(
            "https://example.invalid/retrio.git".to_string(),
            "deadbeef".to_string(),
            "0".repeat(64),
        );
        fs::create_dir_all(
            root.join(
                source.required_files[0]
                    .path
                    .parent()
                    .expect("required file path should have a parent"),
            ),
        )
        .expect("required file parent should be creatable");
        fs::write(root.join(&source.required_files[0].path), b"wrong-bytes")
            .expect("required file should be writable");

        let error =
            verify_required_files(&root, &source).expect_err("mismatched hash should be rejected");

        assert!(error.contains("hash mismatch"));

        fs::remove_dir_all(root).expect("temp root should be removable");
    }
}
