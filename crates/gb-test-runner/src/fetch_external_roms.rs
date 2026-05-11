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

use crate::curated_test_roms::{
    materialize_curated_test_rom_source_families, replace_curated_test_rom_families,
};
use crate::{
    ExternalRomRequiredFile, ExternalRomSource, curated_test_rom_families, external_rom_store_root,
    load_external_rom_source_manifest,
};
static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct FetchedExternalRomSource {
    source: ExternalRomSource,
    temp_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FetchExternalRomsAction {
    ShowHelp,
    Fetch(FetchExternalRomsOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FetchExternalRomsOptions {
    force: bool,
    requested_families: Vec<String>,
}

pub fn fetch_external_roms_help_text() -> &'static str {
    concat!(
        "Usage: cargo run -p gb-test-runner --bin fetch_test_roms -- [--force] [all | family ...]\n",
        "\n",
        "Fetches the pinned upstream ROM source(s) into temporary checkout(s), materializes the curated runnable families under .roms/test/, and removes the raw checkout afterwards.\n",
        "When no family is provided, or when `all` is provided, all curated families are materialized.\n",
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
            let _ = options.force;
            let selected_families = select_curated_families(&options.requested_families)?;
            let filtered_sources =
                filter_sources_for_curated_families(manifest.sources(), &selected_families)?;
            let fetched_sources = fetch_sources_into_temps(&filtered_sources, output)?;
            let result = (|| {
                replace_curated_test_rom_families(workspace_root, &selected_families)?;
                for fetched_source in &fetched_sources {
                    materialize_curated_test_rom_source_families(
                        workspace_root,
                        &fetched_source.source.id,
                        &fetched_source.temp_root,
                        &selected_families,
                    )?;
                    remove_legacy_source_cache(workspace_root, &fetched_source.source)?;
                }
                if selected_families.len() == curated_test_rom_families().len() {
                    writeln_checked(
                        output,
                        &format!(
                            "materialized curated test ROM store into {}",
                            workspace_root.join(crate::TEST_ROM_STORE_DIR).display()
                        ),
                    )?;
                } else {
                    writeln_checked(
                        output,
                        &format!(
                            "materialized curated test ROM families {} into {}",
                            selected_families.join(", "),
                            workspace_root.join(crate::TEST_ROM_STORE_DIR).display()
                        ),
                    )?;
                }
                Ok(())
            })();
            let cleanup = cleanup_fetched_sources(&fetched_sources);
            match (result, cleanup) {
                (Ok(()), Ok(())) => Ok(()),
                (Ok(()), Err(error)) => Err(error),
                (Err(error), Ok(())) => Err(error),
                (Err(error), Err(cleanup_error)) => {
                    Err(format!("{error}; additionally {cleanup_error}"))
                }
            }?;

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
    let mut requested_families = Vec::new();

    for argument in arguments {
        match argument.as_ref() {
            "--force" => force = true,
            "--help" | "-h" => return Ok(FetchExternalRomsAction::ShowHelp),
            other => requested_families.push(other.to_string()),
        }
    }

    if requested_families.len() > 1 && requested_families.iter().any(|family| family == "all") {
        return Err("`all` cannot be combined with explicit curated family names".to_string());
    }
    if requested_families == ["all"] {
        requested_families.clear();
    }

    Ok(FetchExternalRomsAction::Fetch(FetchExternalRomsOptions {
        force,
        requested_families,
    }))
}

fn select_curated_families(requested_families: &[String]) -> Result<Vec<String>, String> {
    let available_families = curated_test_rom_families();
    if requested_families.is_empty() {
        return Ok(available_families);
    }

    let available_family_set = available_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut selected = Vec::with_capacity(requested_families.len());
    for family in requested_families {
        if !available_family_set.contains(family.as_str()) {
            return Err(format!(
                "unknown curated test ROM family {family:?}; available families: {}",
                available_families.join(", ")
            ));
        }
        selected.push(family.clone());
    }

    Ok(selected)
}

fn filter_sources_for_curated_families(
    sources: &[ExternalRomSource],
    selected_families: &[String],
) -> Result<Vec<ExternalRomSource>, String> {
    let filtered_sources = sources
        .iter()
        .filter_map(|source| filter_source_for_curated_families(source, selected_families))
        .collect::<Vec<_>>();

    if filtered_sources.is_empty() {
        return Err(format!(
            "no pinned upstream files matched curated family selection {}",
            selected_families.join(", ")
        ));
    }
    let matched_family_set = filtered_sources
        .iter()
        .flat_map(|source| &source.required_files)
        .filter_map(required_file_curated_family)
        .collect::<BTreeSet<_>>();
    let missing_families = selected_families
        .iter()
        .filter(|family| !matched_family_set.contains(family.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_families.is_empty() {
        return Err(format!(
            "no pinned upstream files matched curated family selection {}",
            missing_families.join(", ")
        ));
    }

    Ok(filtered_sources)
}

fn filter_source_for_curated_families(
    source: &ExternalRomSource,
    selected_families: &[String],
) -> Option<ExternalRomSource> {
    let selected_family_set = selected_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let required_files = source
        .required_files
        .iter()
        .filter(|required_file| {
            required_file_matches_any_family(required_file, &selected_family_set)
        })
        .cloned()
        .collect::<Vec<_>>();

    if required_files.is_empty() {
        return None;
    }

    Some(ExternalRomSource {
        required_files,
        ..source.clone()
    })
}

fn required_file_matches_any_family(
    required_file: &ExternalRomRequiredFile,
    selected_families: &BTreeSet<&str>,
) -> bool {
    required_file_curated_family(required_file)
        .is_some_and(|family| selected_families.contains(family))
}

fn required_file_curated_family(required_file: &ExternalRomRequiredFile) -> Option<&str> {
    required_file
        .family
        .as_deref()
        .or_else(|| required_file_family(&required_file.path))
}

fn fetch_sources_into_temps<W: Write>(
    sources: &[ExternalRomSource],
    output: &mut W,
) -> Result<Vec<FetchedExternalRomSource>, String> {
    let mut fetched_sources = Vec::new();
    for source in sources {
        let temp_root = unique_temp_fetch_root(source);
        if let Err(error) = fetch_source_into_temp(&temp_root, source, output) {
            let current_cleanup =
                remove_directory_if_present(&temp_root, "temporary fetch directory");
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
        fetched_sources.push(FetchedExternalRomSource {
            source: source.clone(),
            temp_root,
        });
    }
    Ok(fetched_sources)
}

fn fetch_source_into_temp<W: Write>(
    temp_root: &Path,
    source: &ExternalRomSource,
    output: &mut W,
) -> Result<(), String> {
    remove_directory_if_present(temp_root, "stale temporary fetch directory")?;
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

fn cleanup_fetched_sources(fetched_sources: &[FetchedExternalRomSource]) -> Result<(), String> {
    for fetched_source in fetched_sources {
        remove_directory_if_present(&fetched_source.temp_root, "temporary fetch directory")?;
    }
    Ok(())
}

fn unique_temp_fetch_root(source: &ExternalRomSource) -> std::path::PathBuf {
    let source_name = source.local_dir.replace('/', "-");
    unique_temp_path(&format!("test-rom-fetch-{source_name}"))
}

fn unique_temp_path(label: &str) -> std::path::PathBuf {
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

fn remove_legacy_source_cache(
    workspace_root: &Path,
    source: &ExternalRomSource,
) -> Result<(), String> {
    let legacy_root = external_rom_store_root(workspace_root).join(&source.local_dir);
    remove_directory_if_present(&legacy_root, "legacy raw test ROM cache directory")?;

    let legacy_store_root = external_rom_store_root(workspace_root);
    if legacy_store_root.exists() {
        let is_empty = fs::read_dir(&legacy_store_root)
            .map_err(|error| {
                format!(
                    "failed to read legacy raw test ROM cache root {}: {error}",
                    legacy_store_root.display()
                )
            })?
            .next()
            .is_none();
        if is_empty {
            fs::remove_dir(&legacy_store_root).map_err(|error| {
                format!(
                    "failed to remove empty legacy raw test ROM cache root {}: {error}",
                    legacy_store_root.display()
                )
            })?;
        }
    }

    Ok(())
}

fn remove_directory_if_present(path: &Path, label: &str) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("failed to remove {label} {}: {error}", path.display()))?;
    }
    Ok(())
}
fn checkout_source_into_temp(temp_root: &Path, source: &ExternalRomSource) -> Result<(), String> {
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
                "failed to remove git metadata from fetched source {} at {}: {error}",
                source.id,
                git_dir.display()
            )
        })?;
    }

    Ok(())
}

fn source_sparse_checkout_paths(source: &ExternalRomSource) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for required_file in &source.required_files {
        unique.insert(required_file_sparse_checkout_path(&required_file.path));
    }
    unique.into_iter().collect()
}

fn required_file_sparse_checkout_path(path: &Path) -> String {
    let mut components = path.components();
    let Some(top_level) = components.next() else {
        return String::new();
    };
    let top_level = top_level.as_os_str().to_string_lossy().into_owned();
    if top_level != "testroms" {
        return top_level;
    }

    let Some(family) = components.next() else {
        return "testroms".to_string();
    };
    format!(
        "testroms/{}",
        family.as_os_str().to_string_lossy().into_owned()
    )
}

fn required_file_family(path: &Path) -> Option<&str> {
    let mut components = path.components();
    if components.next()?.as_os_str() != "testroms" {
        return None;
    }
    components.next()?.as_os_str().to_str()
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
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn scrub_inherited_git_repository_context(command: &mut Command) {
    // Git hooks export repository-scoped variables such as `GIT_DIR` and `GIT_PREFIX`.
    // Fixture repositories and temporary sparse checkouts must not inherit that context.
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
    source: &ExternalRomSource,
) -> Result<(), String> {
    let mut command = Command::new("git");
    command.current_dir(current_dir);
    command.args(args);
    run_git_command(command, source, "git command")
}

fn run_git_command(
    mut command: Command,
    source: &ExternalRomSource,
    label: &str,
) -> Result<(), String> {
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
    use std::fmt::Write as _;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use crate::{
        EXTERNAL_ROM_SOURCE_MANIFEST_PATH, ax6_dmg_extra_suite, cgb_audio_blargg_suite,
        cgb_audio_samesuite_suite, cgb_boot_div_suite, cgb_boot_hwio_suite, cgb_dma_suite,
        cgb_ppu_basic_suite, cgb_ppu_hard_suite, cgb_rtc_suite, cgb_smoke_suite, cgb_speed_suite,
        curated_test_rom_families, curated_test_rom_family_suites, docboy_dmg_extra_suite,
        external_rom_store_root, gbmicrotest_dmg_extra_suite, samesuite_dmg_extra_suite,
        test_rom_store_root,
    };

    use super::{
        FetchExternalRomsAction, FetchExternalRomsOptions, fetch_external_roms_help_text,
        filter_source_for_curated_families, parse_fetch_external_roms_arguments,
        required_file_family, required_file_sparse_checkout_path, run_fetch_external_roms_command,
        select_curated_families, sha256_hex, source_sparse_checkout_paths, verify_required_files,
    };
    use crate::{ExternalRomRequiredFile, ExternalRomSource};

    fn unique_temp_dir(label: &str) -> PathBuf {
        super::unique_temp_path(&format!("fetch-external-roms-{label}"))
    }

    fn commit_upstream_repo(root: &Path) -> String {
        git(&["init", "--no-bare"], root);
        git(&["add", "."], root);
        git_with_env(
            &["commit", "-m", "fixture"],
            root,
            &[
                ("GIT_AUTHOR_EMAIL", "gb-cycle@example.invalid"),
                ("GIT_AUTHOR_NAME", "gb-cycle tests"),
                ("GIT_COMMITTER_EMAIL", "gb-cycle@example.invalid"),
                ("GIT_COMMITTER_NAME", "gb-cycle tests"),
            ],
        );

        let mut command = Command::new("git");
        command.current_dir(root);
        command.args(["rev-parse", "HEAD"]);
        super::scrub_inherited_git_repository_context(&mut command);
        let output = command.output().expect("git rev-parse should spawn");
        assert!(output.status.success(), "git rev-parse should succeed");
        String::from_utf8(output.stdout)
            .expect("git hash should be utf-8")
            .trim()
            .to_string()
    }

    fn write_manifest(workspace_root: &Path, source: &ExternalRomSource) {
        write_manifest_sources(workspace_root, &[source]);
    }

    fn write_manifest_sources(workspace_root: &Path, sources: &[&ExternalRomSource]) {
        let manifest_path = workspace_root.join(EXTERNAL_ROM_SOURCE_MANIFEST_PATH);
        let manifest_parent = manifest_path
            .parent()
            .expect("manifest path should have a parent");
        fs::create_dir_all(manifest_parent).expect("manifest parent should be creatable");

        let mut manifest = "version = 1\n".to_string();
        for source in sources {
            let mut required_files = String::new();
            for required_file in &source.required_files {
                let family = required_file
                    .family
                    .as_ref()
                    .map_or(String::new(), |family| format!("family = {family:?}\n"));
                let rom = required_file.rom.as_ref().map_or(String::new(), |rom| {
                    format!("rom = {:?}\n", rom.display().to_string())
                });
                let _ = write!(
                    &mut required_files,
                    concat!(
                        "\n[[source.required_file]]\n",
                        "path = {:?}\n",
                        "{}",
                        "{}",
                        "sha256 = {:?}\n",
                    ),
                    required_file.path.display().to_string(),
                    family,
                    rom,
                    required_file.sha256,
                );
            }
            let _ = write!(
                &mut manifest,
                concat!(
                    "\n[[source]]\n",
                    "id = {:?}\n",
                    "git_url = {:?}\n",
                    "git_rev = {:?}\n",
                    "local_dir = {:?}\n",
                    "root_env_var = {:?}\n",
                    "{}",
                ),
                source.id,
                source.git_url,
                source.git_rev,
                source.local_dir,
                source.root_env_var,
                required_files,
            );
        }

        fs::write(manifest_path, manifest).expect("manifest should be writable");
    }

    fn git(args: &[&str], current_dir: &Path) {
        git_with_env(args, current_dir, &[]);
    }

    fn git_with_env(args: &[&str], current_dir: &Path, envs: &[(&str, &str)]) {
        let mut command = Command::new("git");
        command.current_dir(current_dir);
        command.envs(envs.iter().copied());
        command.args(args);
        super::scrub_inherited_git_repository_context(&mut command);
        let output = command.output().expect("git command should spawn");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn build_source(git_url: String, git_rev: String, sha256: String) -> ExternalRomSource {
        ExternalRomSource {
            id: "gbemu-shootout".to_string(),
            git_url,
            git_rev,
            local_dir: "gbemu-shootout".to_string(),
            root_env_var: "GB_CYCLE_GBEMU_SHOOTOUT_ROOT".to_string(),
            required_files: vec![ExternalRomRequiredFile {
                path: PathBuf::from("testroms/blargg/cpu_instrs/01-special.gb"),
                family: None,
                rom: None,
                sha256,
            }],
        }
    }

    fn build_curated_source(git_url: String, git_rev: String, root: &Path) -> ExternalRomSource {
        let required_files = curated_test_rom_family_suites()
            .into_iter()
            .chain([
                ax6_dmg_extra_suite(),
                samesuite_dmg_extra_suite(),
                cgb_smoke_suite(),
                cgb_boot_div_suite(),
                cgb_boot_hwio_suite(),
                cgb_speed_suite(),
                cgb_ppu_basic_suite(),
                cgb_ppu_hard_suite(),
                cgb_dma_suite(),
                cgb_rtc_suite(),
                cgb_audio_blargg_suite(),
                cgb_audio_samesuite_suite(),
            ])
            .flat_map(|suite| suite.cases.into_iter().map(|case| case.rom_path))
            .map(|path| ExternalRomRequiredFile {
                path: PathBuf::from("testroms").join(&path),
                family: None,
                rom: None,
                sha256: sha256_hex(
                    &fs::read(root.join("testroms").join(&path))
                        .expect("required curated source file should be readable"),
                ),
            })
            .collect();

        ExternalRomSource {
            id: "gbemu-shootout".to_string(),
            git_url,
            git_rev,
            local_dir: "gbemu-shootout".to_string(),
            root_env_var: "GB_CYCLE_GBEMU_SHOOTOUT_ROOT".to_string(),
            required_files,
        }
    }

    fn write_curated_shootout_repo(root: &Path) {
        for suite in curated_test_rom_family_suites().into_iter().chain([
            ax6_dmg_extra_suite(),
            samesuite_dmg_extra_suite(),
            cgb_smoke_suite(),
            cgb_boot_div_suite(),
            cgb_boot_hwio_suite(),
            cgb_speed_suite(),
            cgb_ppu_basic_suite(),
            cgb_ppu_hard_suite(),
            cgb_dma_suite(),
            cgb_rtc_suite(),
            cgb_audio_blargg_suite(),
            cgb_audio_samesuite_suite(),
        ]) {
            for case in suite.cases {
                let source_path = root.join("testroms").join(&case.rom_path);
                fs::create_dir_all(
                    source_path
                        .parent()
                        .expect("ROM path should always have a parent"),
                )
                .expect("source ROM parent should be creatable");
                fs::write(&source_path, case.id.as_bytes())
                    .expect("source ROM fixture should be writable");
            }
        }
    }

    fn write_required_file(root: &Path, path: &str, bytes: &[u8]) -> String {
        let full_path = root.join(path);
        fs::create_dir_all(
            full_path
                .parent()
                .expect("required file should have a parent"),
        )
        .expect("required file parent should be creatable");
        fs::write(&full_path, bytes).expect("required file should be writable");
        sha256_hex(bytes)
    }

    fn write_docboy_repo(root: &Path) {
        write_required_file(
            root,
            "tests/roms/dmg/samesuite/interrupt/ei_delay_halt.gb",
            b"docboy-ei-delay-halt",
        );
        write_required_file(
            root,
            "tests/results/dmg/samesuite/interrupt/ei_delay_halt.png",
            b"docboy-ei-delay-halt-png",
        );
        write_required_file(
            root,
            "tests/roms/dmg/little-things-gb/double-halt-cancel.gb",
            b"docboy-double-halt-cancel",
        );
        write_required_file(
            root,
            "tests/results/dmg/little-things-gb/double-halt-cancel.png",
            b"docboy-double-halt-cancel-png",
        );
        write_required_file(
            root,
            "tests/roms/dmg/little-things-gb/whichboot.gb",
            b"docboy-whichboot",
        );
        write_required_file(
            root,
            "tests/results/dmg/little-things-gb/whichboot.png",
            b"docboy-whichboot-png",
        );
        for case in gbmicrotest_dmg_extra_suite().cases {
            write_required_file(
                root,
                &format!("tests/roms/dmg/{}", case.rom_path.display()),
                case.id.as_bytes(),
            );
        }
        for case in docboy_dmg_extra_suite().cases {
            write_required_file(
                root,
                &format!("tests/roms/dmg/{}", case.rom_path.display()),
                case.id.as_bytes(),
            );
        }
        for rom_path in crate::curated_test_roms::disabled_curated_rom_paths_for_family("docboy") {
            write_required_file(
                root,
                &format!("tests/roms/dmg/{}", rom_path.display()),
                rom_path.to_string_lossy().as_bytes(),
            );
        }
        for rom in [
            "docboy/serial/serial_two_players_basic_transfer_master.gb",
            "docboy/serial/serial_two_players_basic_transfer_slave.gb",
            "docboy/serial/serial_two_players_basic_transfer_slave_sc_00.gb",
        ] {
            write_required_file(root, &format!("tests/roms/dmg/{rom}"), rom.as_bytes());
        }
    }

    fn build_docboy_source(git_url: String, git_rev: String, root: &Path) -> ExternalRomSource {
        let required_file = |path: &str, family: &str, rom: Option<&str>| ExternalRomRequiredFile {
            path: PathBuf::from(path),
            family: Some(family.to_string()),
            rom: rom.map(PathBuf::from),
            sha256: sha256_hex(
                &fs::read(root.join(path)).expect("required DocBoy file should be readable"),
            ),
        };
        ExternalRomSource {
            id: "docboy".to_string(),
            git_url,
            git_rev,
            local_dir: "docboy".to_string(),
            root_env_var: "GB_CYCLE_DOCBOY_ROOT".to_string(),
            required_files: vec![
                required_file(
                    "tests/roms/dmg/samesuite/interrupt/ei_delay_halt.gb",
                    "samesuite",
                    Some("interrupt/ei_delay_halt.gb"),
                ),
                required_file(
                    "tests/results/dmg/samesuite/interrupt/ei_delay_halt.png",
                    "samesuite",
                    None,
                ),
                required_file(
                    "tests/roms/dmg/little-things-gb/double-halt-cancel.gb",
                    "little-things-gb",
                    Some("double-halt-cancel.gb"),
                ),
                required_file(
                    "tests/results/dmg/little-things-gb/double-halt-cancel.png",
                    "little-things-gb",
                    None,
                ),
                required_file(
                    "tests/roms/dmg/little-things-gb/whichboot.gb",
                    "little-things-gb",
                    Some("whichboot.gb"),
                ),
                required_file(
                    "tests/results/dmg/little-things-gb/whichboot.png",
                    "little-things-gb",
                    None,
                ),
            ]
            .into_iter()
            .chain(gbmicrotest_dmg_extra_suite().cases.into_iter().map(|case| {
                let rom = case
                    .rom_path
                    .strip_prefix("gbmicrotest")
                    .expect("gbmicrotest manifest ROMs should stay under the gbmicrotest family");
                let rom = rom.display().to_string();
                required_file(
                    &format!("tests/roms/dmg/{}", case.rom_path.display()),
                    "gbmicrotest",
                    Some(rom.as_str()),
                )
            }))
            .chain(docboy_dmg_extra_suite().cases.into_iter().map(|case| {
                let rom = case
                    .rom_path
                    .strip_prefix("docboy")
                    .expect("docboy manifest ROMs should stay under the docboy family");
                let rom = rom.display().to_string();
                required_file(
                    &format!("tests/roms/dmg/{}", case.rom_path.display()),
                    "docboy",
                    Some(rom.as_str()),
                )
            }))
            .chain(
                crate::curated_test_roms::disabled_curated_rom_paths_for_family("docboy")
                    .into_iter()
                    .map(|rom_path| {
                        let rom = rom_path
                            .strip_prefix("docboy")
                            .expect("docboy disabled ROMs should stay under the docboy family")
                            .display()
                            .to_string();
                        required_file(
                            &format!("tests/roms/dmg/{}", rom_path.display()),
                            "docboy",
                            Some(rom.as_str()),
                        )
                    }),
            )
            .chain(
                [
                    "serial/serial_two_players_basic_transfer_master.gb",
                    "serial/serial_two_players_basic_transfer_slave.gb",
                    "serial/serial_two_players_basic_transfer_slave_sc_00.gb",
                ]
                .into_iter()
                .map(|rom| {
                    required_file(&format!("tests/roms/dmg/docboy/{rom}"), "docboy", Some(rom))
                }),
            )
            .collect(),
        }
    }

    #[test]
    fn parse_fetch_command_arguments_supports_help_force_and_family_selection() {
        assert_eq!(
            parse_fetch_external_roms_arguments(["--help"]).expect("help should parse"),
            FetchExternalRomsAction::ShowHelp
        );
        assert_eq!(
            parse_fetch_external_roms_arguments(["--force", "blargg", "acid"])
                .expect("fetch args should parse"),
            FetchExternalRomsAction::Fetch(FetchExternalRomsOptions {
                force: true,
                requested_families: vec!["blargg".to_string(), "acid".to_string()],
            })
        );
        assert_eq!(
            parse_fetch_external_roms_arguments(["all"]).expect("all should parse"),
            FetchExternalRomsAction::Fetch(FetchExternalRomsOptions {
                force: false,
                requested_families: Vec::new(),
            })
        );
    }

    #[test]
    fn parse_fetch_command_arguments_rejects_mixed_all_and_explicit_families() {
        let error = parse_fetch_external_roms_arguments(["all", "blargg"])
            .expect_err("mixing all and explicit families should fail");
        assert!(error.contains("cannot be combined"));
    }

    #[test]
    fn select_curated_families_defaults_to_the_full_supported_set() {
        assert_eq!(
            select_curated_families(&[]).expect("default selection should succeed"),
            curated_test_rom_families()
        );
    }

    #[test]
    fn select_curated_families_rejects_unknown_names() {
        let error = select_curated_families(&["unknown".to_string()])
            .expect_err("unknown family should be rejected");
        assert!(error.contains("unknown curated test ROM family"));
        assert!(error.contains("blargg"));
    }

    #[test]
    fn source_sparse_checkout_paths_keep_family_roots_unique() {
        let source = ExternalRomSource {
            id: "source".to_string(),
            git_url: "https://example.invalid/shootout.git".to_string(),
            git_rev: "deadbeef".to_string(),
            local_dir: "gbemu-shootout".to_string(),
            root_env_var: "GB_CYCLE_GBEMU_SHOOTOUT_ROOT".to_string(),
            required_files: vec![
                ExternalRomRequiredFile {
                    path: PathBuf::from("testroms/blargg/cpu_instrs/01-special.gb"),
                    family: None,
                    rom: None,
                    sha256: "a".repeat(64),
                },
                ExternalRomRequiredFile {
                    path: PathBuf::from("testroms/blargg/cpu_instrs/02-interrupts.gb"),
                    family: None,
                    rom: None,
                    sha256: "b".repeat(64),
                },
            ],
        };

        assert_eq!(
            source_sparse_checkout_paths(&source),
            vec!["testroms/blargg".to_string()]
        );
    }

    #[test]
    fn required_file_helpers_resolve_curated_family_and_sparse_checkout_path() {
        let path = PathBuf::from("testroms/mooneye/acceptance/div_timing.gb");
        assert_eq!(required_file_family(&path), Some("mooneye"));
        assert_eq!(
            required_file_sparse_checkout_path(&path),
            "testroms/mooneye"
        );
    }

    #[test]
    fn filter_source_for_curated_families_keeps_only_the_requested_required_files() {
        let source = ExternalRomSource {
            id: "gbemu-shootout".to_string(),
            git_url: "https://example.invalid/shootout.git".to_string(),
            git_rev: "deadbeef".to_string(),
            local_dir: "gbemu-shootout".to_string(),
            root_env_var: "GB_CYCLE_GBEMU_SHOOTOUT_ROOT".to_string(),
            required_files: vec![
                ExternalRomRequiredFile {
                    path: PathBuf::from("testroms/blargg/cpu_instrs/01-special.gb"),
                    family: None,
                    rom: None,
                    sha256: "a".repeat(64),
                },
                ExternalRomRequiredFile {
                    path: PathBuf::from("testroms/mooneye/acceptance/div_timing.gb"),
                    family: None,
                    rom: None,
                    sha256: "b".repeat(64),
                },
                ExternalRomRequiredFile {
                    path: PathBuf::from("tests/roms/dmg/samesuite/interrupt/ei_delay_halt.gb"),
                    family: Some("samesuite".to_string()),
                    rom: Some(PathBuf::from("interrupt/ei_delay_halt.gb")),
                    sha256: "c".repeat(64),
                },
            ],
        };

        let filtered = filter_source_for_curated_families(&source, &["mooneye".to_string()])
            .expect("selected family should filter required files");
        assert_eq!(filtered.required_files.len(), 1);
        assert_eq!(
            filtered.required_files[0].path,
            PathBuf::from("testroms/mooneye/acceptance/div_timing.gb")
        );
    }

    #[test]
    fn fetch_command_materializes_curated_store_and_removes_the_legacy_raw_cache() {
        let workspace_root = unique_temp_dir("materialize");
        let upstream_root = workspace_root.join("upstream");
        let docboy_root = workspace_root.join("docboy-upstream");
        fs::create_dir_all(&upstream_root).expect("upstream root should be creatable");
        write_curated_shootout_repo(&upstream_root);
        write_docboy_repo(&docboy_root);
        let git_rev = commit_upstream_repo(&upstream_root);
        let docboy_rev = commit_upstream_repo(&docboy_root);
        let source =
            build_curated_source(upstream_root.display().to_string(), git_rev, &upstream_root);
        let docboy_source =
            build_docboy_source(docboy_root.display().to_string(), docboy_rev, &docboy_root);
        write_manifest_sources(&workspace_root, &[&source, &docboy_source]);

        let legacy_raw_root = external_rom_store_root(&workspace_root).join(&source.local_dir);
        fs::create_dir_all(&legacy_raw_root).expect("legacy raw root should be creatable");
        fs::write(legacy_raw_root.join("stale.txt"), b"old-cache")
            .expect("legacy raw marker should be writable");

        let mut output = Vec::new();
        run_fetch_external_roms_command(["all"], &workspace_root, &mut output)
            .expect("fetch command should succeed");

        assert!(
            test_rom_store_root(&workspace_root)
                .join("blargg/cpu_instrs/01-special.gb")
                .exists()
        );
        assert!(
            test_rom_store_root(&workspace_root)
                .join("little-things-gb/whichboot.gb")
                .exists()
        );
        assert!(!legacy_raw_root.exists());
        assert!(
            String::from_utf8(output)
                .expect("command output should be utf-8")
                .contains("temporary workspace")
        );

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn fetch_command_can_materialize_only_one_selected_family() {
        let workspace_root = unique_temp_dir("materialize-selected-family");
        let upstream_root = workspace_root.join("upstream");
        fs::create_dir_all(&upstream_root).expect("upstream root should be creatable");
        write_curated_shootout_repo(&upstream_root);
        let git_rev = commit_upstream_repo(&upstream_root);
        let source =
            build_curated_source(upstream_root.display().to_string(), git_rev, &upstream_root);
        write_manifest(&workspace_root, &source);

        let mut output = Vec::new();
        run_fetch_external_roms_command(["blargg"], &workspace_root, &mut output)
            .expect("selected-family fetch command should succeed");

        assert!(
            test_rom_store_root(&workspace_root)
                .join("blargg/cpu_instrs/01-special.gb")
                .exists()
        );
        assert!(!test_rom_store_root(&workspace_root).join("acid").exists());
        assert!(
            String::from_utf8(output)
                .expect("command output should be utf-8")
                .contains("materialized curated test ROM families blargg")
        );

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn fetch_command_materializes_one_family_from_multiple_sources() {
        let workspace_root = unique_temp_dir("materialize-multi-source-family");
        let gbemu_root = workspace_root.join("gbemu-upstream");
        let docboy_root = workspace_root.join("docboy-upstream");
        write_curated_shootout_repo(&gbemu_root);

        let write_required_file = |root: &Path, path: &str, bytes: &[u8]| -> String {
            let full_path = root.join(path);
            fs::create_dir_all(
                full_path
                    .parent()
                    .expect("required file should have a parent"),
            )
            .expect("required file parent should be creatable");
            fs::write(&full_path, bytes).expect("required file should be writable");
            sha256_hex(bytes)
        };
        let sha_for_file = |root: &Path, path: &str| -> String {
            sha256_hex(&fs::read(root.join(path)).expect("required file should be readable"))
        };

        let div_write_sha =
            sha_for_file(&gbemu_root, "testroms/samesuite/apu/div_write_trigger.gb");
        let div_write_10_sha = sha_for_file(
            &gbemu_root,
            "testroms/samesuite/apu/div_write_trigger_10.gb",
        );
        let ei_delay_halt_sha = write_required_file(
            &docboy_root,
            "tests/roms/dmg/samesuite/interrupt/ei_delay_halt.gb",
            b"docboy-ei-delay-halt",
        );
        let ei_delay_halt_png_sha = write_required_file(
            &docboy_root,
            "tests/results/dmg/samesuite/interrupt/ei_delay_halt.png",
            b"docboy-ei-delay-halt-png",
        );

        let gbemu_rev = commit_upstream_repo(&gbemu_root);
        let docboy_rev = commit_upstream_repo(&docboy_root);
        let manifest_path = workspace_root.join(EXTERNAL_ROM_SOURCE_MANIFEST_PATH);
        fs::create_dir_all(
            manifest_path
                .parent()
                .expect("manifest path should have a parent"),
        )
        .expect("manifest parent should be creatable");
        fs::write(
            &manifest_path,
            format!(
                r#"version = 1

[[source]]
id = "gbemu-shootout"
git_url = "{}"
git_rev = "{}"
local_dir = "gbemu-shootout"
root_env_var = "GB_CYCLE_GBEMU_SHOOTOUT_ROOT"

[[source.required_file]]
path = "testroms/samesuite/apu/div_write_trigger.gb"
sha256 = "{}"

[[source.required_file]]
path = "testroms/samesuite/apu/div_write_trigger_10.gb"
sha256 = "{}"

[[source]]
id = "docboy"
git_url = "{}"
git_rev = "{}"
local_dir = "docboy"
root_env_var = "GB_CYCLE_DOCBOY_ROOT"

[[source.required_file]]
path = "tests/roms/dmg/samesuite/interrupt/ei_delay_halt.gb"
family = "samesuite"
rom = "interrupt/ei_delay_halt.gb"
sha256 = "{}"

[[source.required_file]]
path = "tests/results/dmg/samesuite/interrupt/ei_delay_halt.png"
family = "samesuite"
sha256 = "{}"
"#,
                gbemu_root.display(),
                gbemu_rev,
                div_write_sha,
                div_write_10_sha,
                docboy_root.display(),
                docboy_rev,
                ei_delay_halt_sha,
                ei_delay_halt_png_sha,
            ),
        )
        .expect("manifest should be writable");

        let mut output = Vec::new();
        run_fetch_external_roms_command(["samesuite"], &workspace_root, &mut output)
            .expect("multi-source SameSuite fetch should succeed");

        let samesuite_root = test_rom_store_root(&workspace_root).join("samesuite");
        assert!(samesuite_root.join("apu/div_write_trigger.gb").exists());
        assert!(samesuite_root.join("apu/div_write_trigger_10.gb").exists());
        assert_eq!(
            fs::read_to_string(samesuite_root.join("interrupt/ei_delay_halt.gb"))
                .expect("DocBoy SameSuite ROM should materialize"),
            "docboy-ei-delay-halt"
        );
        let output = String::from_utf8(output).expect("command output should be utf-8");
        assert!(output.contains("fetched test ROM source gbemu-shootout"));
        assert!(output.contains("fetched test ROM source docboy"));

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
    fn commit_upstream_repo_keeps_fixture_identity_out_of_local_git_config() {
        let root = unique_temp_dir("fixture-identity");
        fs::create_dir_all(&root).expect("fixture root should be creatable");
        fs::write(root.join("fixture.txt"), b"fixture").expect("fixture file should be writable");

        let _ = commit_upstream_repo(&root);

        let mut command = Command::new("git");
        command.current_dir(&root);
        command.args(["config", "--local", "--list"]);
        super::scrub_inherited_git_repository_context(&mut command);
        let output = command.output().expect("git config should spawn");
        assert!(output.status.success(), "git config should succeed");
        let config = String::from_utf8(output.stdout).expect("git config output should be utf-8");
        assert!(!config.contains("user.name=gb-cycle tests"));
        assert!(!config.contains("user.email=gb-cycle@example.invalid"));

        fs::remove_dir_all(root).expect("fixture root should be removable");
    }

    #[test]
    fn verify_required_files_reports_hash_mismatches() {
        let root = unique_temp_dir("hash-mismatch");
        let source = build_source(
            "https://example.invalid/shootout.git".to_string(),
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
