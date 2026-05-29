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
    materialize_curated_test_rom_source_report_families,
    replace_curated_test_rom_families_for_report,
};
use crate::{
    ExternalRomRequiredFile, ExternalRomSource, GB_EMULATOR_SHOOTOUT_REPORT_ID,
    curated_test_rom_families_for_report, load_external_rom_source_manifest_for_report,
};
static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);
const LEGACY_REPORT_ID: &str = "legacy";

#[derive(Debug)]
struct FetchedExternalRomSource {
    source: ExternalRomSource,
    temp_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FetchTestRomsAction {
    ShowHelp,
    Fetch(FetchTestRomsOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FetchTestRomsOptions {
    force: bool,
    report_id: Option<String>,
    requested_families: Vec<String>,
}

pub fn fetch_test_roms_help_text() -> &'static str {
    concat!(
        "Usage: cargo run -p gb-test-runner --bin fetch_test_roms -- [--force] <report-id> <family> [family ...]\n",
        "\n",
        "Fetches the pinned upstream ROM source(s) into temporary checkout(s), materializes the curated runnable families and upstream oracle fixtures under test/ or test/<report-id>, and removes the raw checkout afterwards.\n",
        "Report ids: `legacy` for the legacy extra/DocBoy inventory, or `gb-emulator-shootout` for promoted GB Emulator Shootout families.\n",
        "At least one explicit family must be provided. `all`, `null`, and empty selections are not accepted.\n",
    )
}

pub fn run_fetch_test_roms_command<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_fetch_test_roms_arguments(arguments)? {
        FetchTestRomsAction::ShowHelp => write_all(output, fetch_test_roms_help_text()),
        FetchTestRomsAction::Fetch(options) => {
            let manifest = load_external_rom_source_manifest_for_report(
                workspace_root,
                options.report_id.as_deref(),
            )
            .map_err(|error| error.to_string())?;
            let _ = options.force;
            let available_families =
                curated_test_rom_families_for_report(options.report_id.as_deref())?;
            let selected_families =
                select_curated_families(&available_families, &options.requested_families)?;
            let filtered_sources =
                filter_sources_for_curated_families(manifest.sources(), &selected_families)?;
            let fetched_sources = fetch_sources_into_temps(&filtered_sources, output)?;
            let result = (|| {
                replace_curated_test_rom_families_for_report(
                    workspace_root,
                    options.report_id.as_deref(),
                    &selected_families,
                )?;
                for fetched_source in &fetched_sources {
                    materialize_curated_test_rom_source_report_families(
                        workspace_root,
                        options.report_id.as_deref(),
                        &fetched_source.source.id,
                        &fetched_source.temp_root,
                        &selected_families,
                    )?;
                }
                let store_root = options.report_id.as_deref().map_or_else(
                    || workspace_root.join(crate::TEST_ROM_STORE_DIR),
                    |report_id| {
                        workspace_root
                            .join(crate::TEST_ROM_STORE_DIR)
                            .join(report_id)
                    },
                );
                if selected_families.len() == available_families.len() {
                    writeln_checked(
                        output,
                        &format!(
                            "materialized curated test ROM store into {}",
                            store_root.display()
                        ),
                    )?;
                } else {
                    writeln_checked(
                        output,
                        &format!(
                            "materialized curated test ROM families {} into {}",
                            selected_families.join(", "),
                            store_root.display()
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

fn parse_fetch_test_roms_arguments<I, S>(arguments: I) -> Result<FetchTestRomsAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut force = false;
    let mut report = None;
    let mut requested_families = Vec::new();

    for argument in arguments {
        match argument.as_ref() {
            "--force" => force = true,
            "--report" => return Err(
                "fetch_test_roms expects the report as the first positional argument; use \"legacy\" or \"gb-emulator-shootout\"".to_string(),
            ),
            "--help" | "-h" => return Ok(FetchTestRomsAction::ShowHelp),
            other if report.is_none() => report = Some(other.to_string()),
            other => requested_families.push(other.to_string()),
        }
    }

    let Some(report) = report else {
        return Err(
            "curated test ROM report must be provided; use \"legacy\" or \"gb-emulator-shootout\""
                .to_string(),
        );
    };
    let report_id = parse_fetch_report_id(&report)?;
    if requested_families.is_empty() {
        return Err(format!(
            "at least one explicit curated test ROM family must be provided after report {report:?}"
        ));
    }
    if let Some(reserved_family) = requested_families
        .iter()
        .find(|family| matches!(family.as_str(), "all" | "null"))
    {
        return Err(format!(
            "{reserved_family:?} is not a valid curated test ROM family selector; provide one or more explicit family names"
        ));
    }

    Ok(FetchTestRomsAction::Fetch(FetchTestRomsOptions {
        force,
        report_id,
        requested_families,
    }))
}

fn parse_fetch_report_id(report: &str) -> Result<Option<String>, String> {
    match report {
        LEGACY_REPORT_ID => Ok(None),
        GB_EMULATOR_SHOOTOUT_REPORT_ID => Ok(Some(GB_EMULATOR_SHOOTOUT_REPORT_ID.to_string())),
        other => Err(format!(
            "unknown curated test ROM report {other:?}; available reports: {LEGACY_REPORT_ID}, {GB_EMULATOR_SHOOTOUT_REPORT_ID}"
        )),
    }
}

fn select_curated_families(
    available_families: &[String],
    requested_families: &[String],
) -> Result<Vec<String>, String> {
    if requested_families.is_empty() {
        return Err("at least one explicit curated test ROM family must be provided".to_string());
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

    use serde::Deserialize;

    use crate::{
        EXTERNAL_ROM_SOURCE_MANIFEST_PATH, GB_EMULATOR_SHOOTOUT_REPORT_ID,
        GB_EMULATOR_SHOOTOUT_SOURCE_MANIFEST_PATH, ax6_dmg_extra_suite, cgb_audio_blargg_suite,
        cgb_audio_samesuite_suite, cgb_boot_div_suite, cgb_boot_hwio_suite, cgb_dma_suite,
        cgb_ppu_basic_suite, cgb_ppu_hard_suite, cgb_rtc_suite, cgb_smoke_suite, cgb_speed_suite,
        cpp_sgb_suite, curated_test_rom_families_for_report, curated_test_rom_family_suites,
        docboy_cgb_dmg_ext_extra_suite, docboy_cgb_dmg_extra_suite, docboy_cgb_extra_suite,
        docboy_dmg_extra_suite, gbmicrotest_dmg_extra_suite, magen_cgb_extra_suite,
        mooneye_sgb_boot_regs_extra_suite, samesuite_cgb_extra_suite, samesuite_dmg_extra_suite,
        samesuite_sgb_suite, test_rom_store_root, test_rom_store_root_for_report,
    };

    use super::{
        FetchTestRomsAction, FetchTestRomsOptions, fetch_test_roms_help_text,
        filter_source_for_curated_families, parse_fetch_test_roms_arguments, required_file_family,
        required_file_sparse_checkout_path, run_fetch_test_roms_command, select_curated_families,
        sha256_hex, source_sparse_checkout_paths, verify_required_files,
    };
    use crate::{ExternalRomRequiredFile, ExternalRomSource};

    fn unique_temp_dir(label: &str) -> PathBuf {
        super::unique_temp_path(&format!("fetch-test-roms-{label}"))
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
        write_manifest_sources_at(
            &workspace_root.join(EXTERNAL_ROM_SOURCE_MANIFEST_PATH),
            sources,
        );
    }

    fn write_report_manifest_sources(workspace_root: &Path, sources: &[&ExternalRomSource]) {
        write_manifest_sources_at(
            &workspace_root.join(GB_EMULATOR_SHOOTOUT_SOURCE_MANIFEST_PATH),
            sources,
        );
    }

    fn write_manifest_sources_at(manifest_path: &Path, sources: &[&ExternalRomSource]) {
        let manifest_parent = manifest_path
            .parent()
            .expect("manifest path should have a parent");
        fs::create_dir_all(manifest_parent).expect("manifest parent should be creatable");

        let mut manifest = String::new();
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
                let target = required_file
                    .target
                    .as_ref()
                    .map_or(String::new(), |target| {
                        format!("target = {:?}\n", target.display().to_string())
                    });
                let _ = write!(
                    &mut required_files,
                    concat!(
                        "\n[[source.required_file]]\n",
                        "path = {:?}\n",
                        "{}",
                        "{}",
                        "{}",
                        "sha256 = {:?}\n",
                    ),
                    required_file.path.display().to_string(),
                    family,
                    rom,
                    target,
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
                    "{}",
                ),
                source.id, source.git_url, source.git_rev, source.local_dir, required_files,
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
            required_files: vec![ExternalRomRequiredFile {
                path: PathBuf::from("testroms/blargg/cpu_instrs/01-special.gb"),
                family: None,
                rom: None,
                target: None,
                sha256,
            }],
        }
    }

    fn build_curated_source(git_url: String, git_rev: String, root: &Path) -> ExternalRomSource {
        let mut required_files = curated_test_rom_family_suites()
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
                samesuite_cgb_extra_suite(),
                samesuite_sgb_suite(),
                mooneye_sgb_boot_regs_extra_suite(),
                cpp_sgb_suite(),
            ])
            .flat_map(|suite| suite.cases.into_iter().map(|case| case.rom_path))
            .map(|path| {
                let source_path = gbemu_test_fixture_source_path(&path);
                let (family, rom) = gbemu_test_fixture_required_file_alias(&path);
                ExternalRomRequiredFile {
                    sha256: sha256_hex(
                        &fs::read(root.join(&source_path))
                            .expect("required curated source file should be readable"),
                    ),
                    path: source_path,
                    family,
                    rom,
                    target: None,
                }
            })
            .collect::<Vec<_>>();
        required_files.extend(report_source_required_target_files().into_iter().map(
            |required_file| {
                ExternalRomRequiredFile {
                    sha256: sha256_hex(
                        &fs::read(root.join(&required_file.path))
                            .expect("required report fixture source file should be readable"),
                    ),
                    path: required_file.path,
                    family: required_file.family,
                    rom: None,
                    target: required_file.target,
                }
            },
        ));

        ExternalRomSource {
            id: "gbemu-shootout".to_string(),
            git_url,
            git_rev,
            local_dir: "gbemu-shootout".to_string(),
            required_files,
        }
    }

    #[derive(Debug, Deserialize)]
    struct TestSourceManifest {
        #[serde(rename = "source")]
        sources: Vec<TestSource>,
    }

    #[derive(Debug, Deserialize)]
    struct TestSource {
        id: String,
        #[serde(default, rename = "required_file")]
        required_files: Vec<TestRequiredFile>,
    }

    #[derive(Debug, Deserialize)]
    struct TestRequiredFile {
        path: PathBuf,
        family: Option<String>,
        target: Option<PathBuf>,
    }

    fn report_source_required_target_files() -> Vec<TestRequiredFile> {
        let parsed: TestSourceManifest =
            toml::from_str(include_str!("../data/gb-emulator-shootout/sources.toml"))
                .expect("report source manifest should parse in fetch tests");
        parsed
            .sources
            .into_iter()
            .filter(|source| source.id == "gbemu-shootout")
            .flat_map(|source| source.required_files)
            .filter(|required_file| required_file.target.is_some())
            .collect()
    }

    fn gbemu_test_fixture_source_path(rom_path: &Path) -> PathBuf {
        let rom_path = crate::curated_test_roms::rom_path_without_store_prefix(rom_path);
        let mut components = rom_path.components();
        if components
            .next()
            .is_some_and(|component| component.as_os_str() == "ashiepaws")
        {
            return PathBuf::from("testroms/ashiepaws").join(components.collect::<PathBuf>());
        }

        PathBuf::from("testroms").join(rom_path)
    }

    fn gbemu_test_fixture_required_file_alias(
        rom_path: &Path,
    ) -> (Option<String>, Option<PathBuf>) {
        let rom_path = crate::curated_test_roms::rom_path_without_store_prefix(rom_path);
        let mut components = rom_path.components();
        if components
            .next()
            .is_some_and(|component| component.as_os_str() == "ashiepaws")
        {
            return (
                Some("ashiepaws".to_string()),
                Some(components.collect::<PathBuf>()),
            );
        }

        (None, None)
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
            samesuite_cgb_extra_suite(),
            samesuite_sgb_suite(),
            mooneye_sgb_boot_regs_extra_suite(),
            cpp_sgb_suite(),
        ]) {
            for case in suite.cases {
                let source_path = root.join(gbemu_test_fixture_source_path(&case.rom_path));
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
        for required_file in report_source_required_target_files() {
            let source_path = root.join(&required_file.path);
            fs::create_dir_all(
                source_path
                    .parent()
                    .expect("report fixture source path should always have a parent"),
            )
            .expect("report fixture source parent should be creatable");
            fs::write(
                &source_path,
                format!(
                    "fixture:{}",
                    required_file
                        .target
                        .expect("target-filtered fixture should have a target")
                        .display()
                )
                .as_bytes(),
            )
            .expect("report fixture source should be writable");
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

    fn strip_curated_store_prefix<'a>(rom_path: &'a Path, family: &str) -> &'a Path {
        let rom_path = crate::curated_test_roms::rom_path_without_store_prefix(rom_path);
        rom_path
            .strip_prefix(crate::curated_test_roms::curated_family_store_prefix(
                family,
            ))
            .unwrap_or_else(|_| {
                panic!(
                    "{family} manifest ROM {} should stay under its curated store prefix",
                    rom_path.display()
                )
            })
    }

    fn docboy_cgb_source_rom_path(rom: &str) -> String {
        if rom.starts_with("mattcurrie/") {
            format!("tests/roms/cgb/{rom}")
        } else {
            format!("tests/roms/cgb/docboy/{rom}")
        }
    }

    fn docboy_cgb_dmg_source_rom_path(rom: &str) -> String {
        format!("tests/roms/cgb_dmg_mode/docboy/{rom}")
    }

    fn docboy_cgb_dmg_ext_source_rom_path(rom: &str) -> String {
        format!("tests/roms/cgb_dmg_ext_mode/docboy/{rom}")
    }

    fn docboy_samesuite_cgb_roms() -> &'static [&'static str] {
        &[
            "apu/channel_1/channel_1_align_12-cgbE.gb",
            "apu/channel_1/channel_1_align_25-cgbE.gb",
            "apu/channel_1/channel_1_align_75-cgbE.gb",
            "apu/channel_1/channel_1_freq_change_timing-cgbDE.gb",
            "apu/channel_1/channel_1_stop_div-cgbE.gb",
            "apu/channel_1/channel_1_sweep-cgbE.gb",
            "apu/channel_1/channel_1_sweep_restart-cgbE.gb",
            "apu/channel_1/channel_1_sweep_restart_2-cgbE.gb",
            "apu/channel_1/channel_1_volume_div-cgbE.gb",
            "apu/channel_3/channel_3_wave_ram_dac_on_rw.gb",
        ]
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
        for rom in docboy_samesuite_cgb_roms() {
            write_required_file(
                root,
                &format!("tests/roms/cgb/samesuite/{rom}"),
                rom.as_bytes(),
            );
            let png = rom.replace(".gb", ".png");
            write_required_file(
                root,
                &format!("tests/results/cgb/samesuite/{png}"),
                png.as_bytes(),
            );
        }
        for case in magen_cgb_extra_suite().cases {
            let rom = strip_curated_store_prefix(&case.rom_path, "magen");
            let rom = rom.display().to_string();
            write_required_file(
                root,
                &format!("tests/roms/cgb/magen/{rom}"),
                case.id.as_bytes(),
            );
        }
        for fixture in [
            "bg_oam_priority.png",
            "green.png",
            "oam_internal_priority.png",
        ] {
            write_required_file(
                root,
                &format!("tests/results/cgb/magen/{fixture}"),
                fixture.as_bytes(),
            );
        }
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
        write_required_file(
            root,
            "tests/results/cgb/little-things-gb/whichboot.png",
            b"docboy-cgb-whichboot-png",
        );
        for case in gbmicrotest_dmg_extra_suite().cases {
            let rom_path = crate::curated_test_roms::rom_path_without_store_prefix(&case.rom_path);
            write_required_file(
                root,
                &format!("tests/roms/dmg/{}", rom_path.display()),
                case.id.as_bytes(),
            );
        }
        for case in docboy_dmg_extra_suite().cases {
            let rom = strip_curated_store_prefix(&case.rom_path, "docboy-dmg");
            write_required_file(
                root,
                &format!("tests/roms/dmg/docboy/{}", rom.display()),
                case.id.as_bytes(),
            );
        }
        for case in docboy_cgb_extra_suite().cases {
            let rom = strip_curated_store_prefix(&case.rom_path, "docboy-cgb");
            write_required_file(
                root,
                &docboy_cgb_source_rom_path(&rom.display().to_string()),
                case.id.as_bytes(),
            );
        }
        for case in docboy_cgb_dmg_extra_suite().cases {
            let rom = strip_curated_store_prefix(&case.rom_path, "docboy-cgb-dmg");
            write_required_file(
                root,
                &docboy_cgb_dmg_source_rom_path(&rom.display().to_string()),
                case.id.as_bytes(),
            );
        }
        for case in docboy_cgb_dmg_ext_extra_suite().cases {
            let rom = strip_curated_store_prefix(&case.rom_path, "docboy-cgb-dmg-ext");
            write_required_file(
                root,
                &docboy_cgb_dmg_ext_source_rom_path(&rom.display().to_string()),
                case.id.as_bytes(),
            );
        }
        for rom_path in
            crate::curated_test_roms::disabled_curated_rom_paths_for_family("docboy-dmg")
        {
            let rom = strip_curated_store_prefix(&rom_path, "docboy-dmg");
            write_required_file(
                root,
                &format!("tests/roms/dmg/docboy/{}", rom.display()),
                rom_path.to_string_lossy().as_bytes(),
            );
        }
        for rom_path in
            crate::curated_test_roms::disabled_curated_rom_paths_for_family("docboy-cgb")
        {
            let rom = strip_curated_store_prefix(&rom_path, "docboy-cgb");
            write_required_file(
                root,
                &docboy_cgb_source_rom_path(&rom.display().to_string()),
                rom_path.to_string_lossy().as_bytes(),
            );
        }
        for rom_path in
            crate::curated_test_roms::disabled_curated_rom_paths_for_family("docboy-cgb-dmg")
        {
            let rom = strip_curated_store_prefix(&rom_path, "docboy-cgb-dmg");
            write_required_file(
                root,
                &docboy_cgb_dmg_source_rom_path(&rom.display().to_string()),
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
            target: None,
            sha256: sha256_hex(
                &fs::read(root.join(path)).expect("required DocBoy file should be readable"),
            ),
        };
        ExternalRomSource {
            id: "docboy".to_string(),
            git_url,
            git_rev,
            local_dir: "docboy".to_string(),
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
            ]
            .into_iter()
            .chain(docboy_samesuite_cgb_roms().iter().flat_map(|rom| {
                let png = rom.replace(".gb", ".png");
                [
                    required_file(
                        &format!("tests/roms/cgb/samesuite/{rom}"),
                        "samesuite",
                        Some(rom),
                    ),
                    required_file(
                        &format!("tests/results/cgb/samesuite/{png}"),
                        "samesuite",
                        None,
                    ),
                ]
            }))
            .chain(magen_cgb_extra_suite().cases.into_iter().map(|case| {
                let rom = strip_curated_store_prefix(&case.rom_path, "magen");
                let rom = rom.display().to_string();
                required_file(
                    &format!("tests/roms/cgb/magen/{rom}"),
                    "magen",
                    Some(rom.as_str()),
                )
            }))
            .chain(
                [
                    "bg_oam_priority.png",
                    "green.png",
                    "oam_internal_priority.png",
                ]
                .into_iter()
                .map(|fixture| {
                    required_file(&format!("tests/results/cgb/magen/{fixture}"), "magen", None)
                }),
            )
            .chain([
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
                required_file(
                    "tests/results/cgb/little-things-gb/whichboot.png",
                    "little-things-gb",
                    None,
                ),
            ])
            .chain(gbmicrotest_dmg_extra_suite().cases.into_iter().map(|case| {
                let rom_path =
                    crate::curated_test_roms::rom_path_without_store_prefix(&case.rom_path);
                let rom = rom_path
                    .strip_prefix("gbmicrotest")
                    .expect("gbmicrotest manifest ROMs should stay under the gbmicrotest family");
                let rom = rom.display().to_string();
                required_file(
                    &format!("tests/roms/dmg/{}", rom_path.display()),
                    "gbmicrotest",
                    Some(rom.as_str()),
                )
            }))
            .chain(docboy_dmg_extra_suite().cases.into_iter().map(|case| {
                let rom = strip_curated_store_prefix(&case.rom_path, "docboy-dmg");
                let rom = rom.display().to_string();
                required_file(
                    &format!("tests/roms/dmg/docboy/{rom}"),
                    "docboy-dmg",
                    Some(rom.as_str()),
                )
            }))
            .chain(docboy_cgb_extra_suite().cases.into_iter().map(|case| {
                let rom = strip_curated_store_prefix(&case.rom_path, "docboy-cgb");
                let rom = rom.display().to_string();
                required_file(
                    &docboy_cgb_source_rom_path(&rom),
                    "docboy-cgb",
                    Some(rom.as_str()),
                )
            }))
            .chain(docboy_cgb_dmg_extra_suite().cases.into_iter().map(|case| {
                let rom = strip_curated_store_prefix(&case.rom_path, "docboy-cgb-dmg");
                let rom = rom.display().to_string();
                required_file(
                    &docboy_cgb_dmg_source_rom_path(&rom),
                    "docboy-cgb-dmg",
                    Some(rom.as_str()),
                )
            }))
            .chain(
                docboy_cgb_dmg_ext_extra_suite()
                    .cases
                    .into_iter()
                    .map(|case| {
                        let rom = strip_curated_store_prefix(&case.rom_path, "docboy-cgb-dmg-ext");
                        let rom = rom.display().to_string();
                        required_file(
                            &docboy_cgb_dmg_ext_source_rom_path(&rom),
                            "docboy-cgb-dmg-ext",
                            Some(rom.as_str()),
                        )
                    }),
            )
            .chain(
                crate::curated_test_roms::disabled_curated_rom_paths_for_family("docboy-dmg")
                    .into_iter()
                    .map(|rom_path| {
                        let rom = strip_curated_store_prefix(&rom_path, "docboy-dmg")
                            .display()
                            .to_string();
                        required_file(
                            &format!("tests/roms/dmg/docboy/{rom}"),
                            "docboy-dmg",
                            Some(rom.as_str()),
                        )
                    }),
            )
            .chain(
                crate::curated_test_roms::disabled_curated_rom_paths_for_family("docboy-cgb")
                    .into_iter()
                    .map(|rom_path| {
                        let rom = strip_curated_store_prefix(&rom_path, "docboy-cgb")
                            .display()
                            .to_string();
                        required_file(
                            &docboy_cgb_source_rom_path(&rom),
                            "docboy-cgb",
                            Some(rom.as_str()),
                        )
                    }),
            )
            .chain(
                crate::curated_test_roms::disabled_curated_rom_paths_for_family("docboy-cgb-dmg")
                    .into_iter()
                    .map(|rom_path| {
                        let rom = strip_curated_store_prefix(&rom_path, "docboy-cgb-dmg")
                            .display()
                            .to_string();
                        required_file(
                            &docboy_cgb_dmg_source_rom_path(&rom),
                            "docboy-cgb-dmg",
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
                    required_file(
                        &format!("tests/roms/dmg/docboy/{rom}"),
                        "docboy-dmg",
                        Some(rom),
                    )
                }),
            )
            .collect(),
        }
    }

    #[test]
    fn parse_fetch_command_arguments_supports_help_force_and_family_selection() {
        assert_eq!(
            parse_fetch_test_roms_arguments(["--help"]).expect("help should parse"),
            FetchTestRomsAction::ShowHelp
        );
        assert_eq!(
            parse_fetch_test_roms_arguments(["--force", "legacy", "blargg", "acid"])
                .expect("fetch args should parse"),
            FetchTestRomsAction::Fetch(FetchTestRomsOptions {
                force: true,
                report_id: None,
                requested_families: vec!["blargg".to_string(), "acid".to_string()],
            })
        );
        assert_eq!(
            parse_fetch_test_roms_arguments(["gb-emulator-shootout", "acid"])
                .expect("report fetch args should parse"),
            FetchTestRomsAction::Fetch(FetchTestRomsOptions {
                force: false,
                report_id: Some("gb-emulator-shootout".to_string()),
                requested_families: vec!["acid".to_string()],
            })
        );
    }

    #[test]
    fn parse_fetch_command_arguments_rejects_missing_and_reserved_family_selectors() {
        let error = parse_fetch_test_roms_arguments(std::iter::empty::<&str>())
            .expect_err("missing report selection should fail");
        assert!(error.contains("curated test ROM report must be provided"));

        let error = parse_fetch_test_roms_arguments(["legacy", "all"])
            .expect_err("all selector should fail");
        assert!(error.contains("not a valid curated test ROM family selector"));

        let error = parse_fetch_test_roms_arguments(["legacy", "null"])
            .expect_err("null selector should fail");
        assert!(error.contains("not a valid curated test ROM family selector"));

        let error =
            parse_fetch_test_roms_arguments(["legacy"]).expect_err("missing family should fail");
        assert!(error.contains("at least one explicit curated test ROM family"));

        let error = parse_fetch_test_roms_arguments(["--report", "gb-emulator-shootout", "acid"])
            .expect_err("legacy --report option should fail");
        assert!(error.contains("first positional argument"));

        let error = parse_fetch_test_roms_arguments(["ax6", "rtc3test"])
            .expect_err("unknown report should fail");
        assert!(error.contains("unknown curated test ROM report"));
    }

    #[test]
    fn select_curated_families_requires_explicit_names() {
        let error =
            select_curated_families(&curated_test_rom_families_for_report(None).unwrap(), &[])
                .expect_err("empty family selection should fail");
        assert!(error.contains("at least one explicit curated test ROM family"));
    }

    #[test]
    fn select_curated_families_rejects_unknown_names() {
        let available_families = curated_test_rom_families_for_report(None).unwrap();
        let error = select_curated_families(&available_families, &["unknown".to_string()])
            .expect_err("unknown family should be rejected");
        assert!(error.contains("unknown curated test ROM family"));
        assert!(error.contains("samesuite"));
        assert!(!error.contains("blargg"));
    }

    #[test]
    fn select_curated_families_requires_the_report_for_promoted_families() {
        let available_families = curated_test_rom_families_for_report(None).unwrap();
        let error = select_curated_families(&available_families, &["blargg".to_string()])
            .expect_err("promoted family should require an explicit report");
        assert!(error.contains("unknown curated test ROM family"));
        assert!(!error.contains("acid"));

        assert_eq!(
            select_curated_families(
                &curated_test_rom_families_for_report(Some(GB_EMULATOR_SHOOTOUT_REPORT_ID))
                    .unwrap(),
                &["blargg".to_string()],
            )
            .expect("promoted family should be available through the report"),
            vec!["blargg".to_string()]
        );
    }

    #[test]
    fn source_sparse_checkout_paths_keep_family_roots_unique() {
        let source = ExternalRomSource {
            id: "source".to_string(),
            git_url: "https://example.invalid/shootout.git".to_string(),
            git_rev: "deadbeef".to_string(),
            local_dir: "gbemu-shootout".to_string(),
            required_files: vec![
                ExternalRomRequiredFile {
                    path: PathBuf::from("testroms/blargg/cpu_instrs/01-special.gb"),
                    family: None,
                    rom: None,
                    target: None,
                    sha256: "a".repeat(64),
                },
                ExternalRomRequiredFile {
                    path: PathBuf::from("testroms/blargg/cpu_instrs/02-interrupts.gb"),
                    family: None,
                    rom: None,
                    target: None,
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
            required_files: vec![
                ExternalRomRequiredFile {
                    path: PathBuf::from("testroms/blargg/cpu_instrs/01-special.gb"),
                    family: None,
                    rom: None,
                    target: None,
                    sha256: "a".repeat(64),
                },
                ExternalRomRequiredFile {
                    path: PathBuf::from("testroms/mooneye/acceptance/div_timing.gb"),
                    family: None,
                    rom: None,
                    target: None,
                    sha256: "b".repeat(64),
                },
                ExternalRomRequiredFile {
                    path: PathBuf::from("tests/roms/dmg/samesuite/interrupt/ei_delay_halt.gb"),
                    family: Some("samesuite".to_string()),
                    rom: Some(PathBuf::from("interrupt/ei_delay_halt.gb")),
                    target: None,
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
    fn fetch_command_materializes_legacy_store_when_all_families_are_explicit() {
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

        let families = curated_test_rom_families_for_report(None)
            .expect("legacy families should be available");
        let mut output = Vec::new();
        run_fetch_test_roms_command(
            std::iter::once("legacy".to_string()).chain(families),
            &workspace_root,
            &mut output,
        )
        .expect("fetch command should succeed");

        assert!(
            test_rom_store_root(&workspace_root)
                .join("ax6/rtc3test-1.gb")
                .exists()
        );
        assert!(!test_rom_store_root(&workspace_root).join("blargg").exists());
        assert!(
            test_rom_store_root(&workspace_root)
                .join("little-things-gb/whichboot.gb")
                .exists()
        );
        assert!(
            String::from_utf8(output)
                .expect("command output should be utf-8")
                .contains("temporary workspace")
        );

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn fetch_command_rejects_missing_report_before_loading_manifest() {
        let workspace_root = unique_temp_dir("missing-report-selection");
        fs::create_dir_all(&workspace_root).expect("workspace root should be creatable");
        let mut output = Vec::new();

        let error =
            run_fetch_test_roms_command(std::iter::empty::<&str>(), &workspace_root, &mut output)
                .expect_err("missing report selection should fail before manifest loading");

        assert!(output.is_empty());
        assert!(error.contains("curated test ROM report must be provided"));
        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn fetch_command_rejects_empty_family_selection_before_loading_manifest() {
        let workspace_root = unique_temp_dir("empty-family-selection");
        fs::create_dir_all(&workspace_root).expect("workspace root should be creatable");
        let mut output = Vec::new();

        let error = run_fetch_test_roms_command(["legacy"], &workspace_root, &mut output)
            .expect_err("missing family selection should fail before manifest loading");

        assert!(output.is_empty());
        assert!(error.contains("at least one explicit curated test ROM family"));
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
        run_fetch_test_roms_command(["legacy", "ax6"], &workspace_root, &mut output)
            .expect("selected-family fetch command should succeed");

        assert!(
            test_rom_store_root(&workspace_root)
                .join("ax6/rtc3test-1.gb")
                .exists()
        );
        assert!(!test_rom_store_root(&workspace_root).join("acid").exists());
        assert!(!test_rom_store_root(&workspace_root).join("blargg").exists());
        assert!(
            String::from_utf8(output)
                .expect("command output should be utf-8")
                .contains("materialized curated test ROM families ax6")
        );

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn fetch_command_materializes_selected_report_family_below_the_report_store() {
        let workspace_root = unique_temp_dir("materialize-report-family");
        let upstream_root = workspace_root.join("upstream");
        fs::create_dir_all(&upstream_root).expect("upstream root should be creatable");
        write_curated_shootout_repo(&upstream_root);
        let git_rev = commit_upstream_repo(&upstream_root);
        let source =
            build_curated_source(upstream_root.display().to_string(), git_rev, &upstream_root);
        write_report_manifest_sources(&workspace_root, &[&source]);

        let mut output = Vec::new();
        run_fetch_test_roms_command(
            [GB_EMULATOR_SHOOTOUT_REPORT_ID, "acid"],
            &workspace_root,
            &mut output,
        )
        .expect("report selected-family fetch command should succeed");

        let report_store_root =
            test_rom_store_root_for_report(&workspace_root, GB_EMULATOR_SHOOTOUT_REPORT_ID);
        assert!(report_store_root.join("acid/which.gb").exists());
        assert!(report_store_root.join("acid/dmg-acid2.gb").exists());
        assert!(report_store_root.join("acid/dmg-acid2.png").exists());
        assert!(report_store_root.join("acid/cgb-acid2.png").exists());
        assert!(!test_rom_store_root(&workspace_root).join("acid").exists());
        assert!(
            String::from_utf8(output)
                .expect("command output should be utf-8")
                .contains(&format!(
                    "materialized curated test ROM families acid into {}",
                    report_store_root.display()
                ))
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
        let mut docboy_samesuite_cgb_required_files = String::new();
        for rom in docboy_samesuite_cgb_roms() {
            let rom_path = format!("tests/roms/cgb/samesuite/{rom}");
            let rom_sha = write_required_file(&docboy_root, &rom_path, rom.as_bytes());
            let png = rom.replace(".gb", ".png");
            let png_path = format!("tests/results/cgb/samesuite/{png}");
            let png_sha = write_required_file(&docboy_root, &png_path, png.as_bytes());
            let _ = write!(
                &mut docboy_samesuite_cgb_required_files,
                concat!(
                    "\n[[source.required_file]]\n",
                    "path = {:?}\n",
                    "family = \"samesuite\"\n",
                    "rom = {:?}\n",
                    "sha256 = {:?}\n",
                    "\n[[source.required_file]]\n",
                    "path = {:?}\n",
                    "family = \"samesuite\"\n",
                    "sha256 = {:?}\n",
                ),
                rom_path, rom, rom_sha, png_path, png_sha,
            );
        }

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
                r#"[[source]]
id = "gbemu-shootout"
git_url = "{}"
git_rev = "{}"
local_dir = "gbemu-shootout"

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

[[source.required_file]]
path = "tests/roms/dmg/samesuite/interrupt/ei_delay_halt.gb"
family = "samesuite"
rom = "interrupt/ei_delay_halt.gb"
sha256 = "{}"

[[source.required_file]]
path = "tests/results/dmg/samesuite/interrupt/ei_delay_halt.png"
family = "samesuite"
sha256 = "{}"
{}
"#,
                gbemu_root.display(),
                gbemu_rev,
                div_write_sha,
                div_write_10_sha,
                docboy_root.display(),
                docboy_rev,
                ei_delay_halt_sha,
                ei_delay_halt_png_sha,
                docboy_samesuite_cgb_required_files,
            ),
        )
        .expect("manifest should be writable");

        let mut output = Vec::new();
        run_fetch_test_roms_command(["legacy", "samesuite"], &workspace_root, &mut output)
            .expect("multi-source SameSuite fetch should succeed");

        let samesuite_root = test_rom_store_root(&workspace_root).join("samesuite");
        assert!(samesuite_root.join("apu/div_write_trigger.gb").exists());
        assert!(samesuite_root.join("apu/div_write_trigger_10.gb").exists());
        assert!(!samesuite_root.join("sgb/command_mlt_req.gb").exists());
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

        run_fetch_test_roms_command(["--help"], &workspace_root, &mut output)
            .expect("help command should succeed");

        assert_eq!(
            String::from_utf8(output).expect("command output should be utf-8"),
            fetch_test_roms_help_text()
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
