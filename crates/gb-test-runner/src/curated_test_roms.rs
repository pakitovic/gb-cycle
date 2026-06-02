use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gb_core::{ConsoleModel, HardwareRevision, HostPlatform, JoypadButton, StartupMode};
use serde::{Deserialize, Serialize};

use crate::external_roms::{
    DOCBOY_REPORT_ID, GB_EMULATOR_SHOOTOUT_REPORT_ID, GBMICROTEST_REPORT_ID,
};
use crate::manifest_fixture::ManifestFixtureField;
use crate::{
    CaptureKind, CapturePlan, ExecutionMode, ExecutionStopCondition, ExternalStimulus,
    ExternalStimulusAction, FailureArtifactPolicy, InformationalCaptureKind, MemoryByteExpectation,
    MemoryTextOutputSpec, PassCondition, RomSuite, RomSuiteReport, RomTestCase, Timeout,
};

pub const TEST_ROM_STORE_DIR: &str = "test";
pub const TEST_ROM_ROOT_ENV_VAR: &str = "GB_CYCLE_TEST_ROM_ROOT";
pub const TEST_ROM_DOCBOY_REPORT_DIR: &str = DOCBOY_REPORT_ID;
pub const TEST_ROM_GB_EMULATOR_SHOOTOUT_REPORT_DIR: &str = GB_EMULATOR_SHOOTOUT_REPORT_ID;
pub const TEST_ROM_GBMICROTEST_REPORT_DIR: &str = GBMICROTEST_REPORT_ID;
pub const TEST_ROM_REPORT_FILE_NAME: &str = "test-report.md";
pub const TEST_ROM_EXTRA_REPORT_FILE_NAME: &str = "test-report-extra.md";
pub const TEST_ROM_DOCBOY_REPORT_FILE_NAME: &str = TEST_ROM_REPORT_FILE_NAME;
pub const TEST_ROM_GBMICROTEST_REPORT_FILE_NAME: &str = TEST_ROM_REPORT_FILE_NAME;

const TEST_ROM_STATUS_DIR_NAME: &str = ".status";
const GBEMU_SHOOTOUT_SOURCE_ID: &str = "gbemu-shootout";
const GBEMU_SHOOTOUT_TESTROMS_DIR: &str = "testroms";
const REPORT_STATUS_PASS_EMOJI: &str = "✅";
const REPORT_STATUS_FAIL_EMOJI: &str = "❌";
const REPORT_STATUS_INFO_EMOJI: &str = "ℹ️";
static CURATED_TEST_ROM_MANIFEST_CACHE: OnceLock<Vec<CuratedTestRomManifest>> = OnceLock::new();
static CURATED_SOURCE_ROM_PATH_CACHE: OnceLock<Vec<(String, PathBuf)>> = OnceLock::new();
static CURATED_SOURCE_ROM_ORDER_CACHE: OnceLock<BTreeMap<(String, PathBuf), usize>> =
    OnceLock::new();
const CURATED_TEST_ROM_REPORT_FAMILY_ORDER: [&str; 9] = [
    "acid",
    "blargg",
    "daid",
    "ax6",
    "mooneye",
    "samesuite",
    "ashiepaws",
    "cpp",
    "mealybug-tearoom-tests",
];
const CURATED_TEST_ROM_EXTRA_REPORT_FAMILY_ORDER: [&str; 6] = [
    "ax6",
    "mooneye",
    "samesuite",
    "magen",
    "mealybug-tearoom-tests",
    "little-things-gb",
];
const CURATED_TEST_ROM_DOCBOY_REPORT_FAMILY_ORDER: [&str; 4] = [
    "docboy-dmg",
    "docboy-cgb",
    "docboy-cgb-dmg",
    "docboy-cgb-dmg-ext",
];
const CURATED_TEST_ROM_GBMICROTEST_REPORT_FAMILY_ORDER: [&str; 1] = ["gbmicrotest"];
const EXTRA_CURATED_TEST_ROM_REPORT_SUITE_NAMES: [&str; 10] = [
    "ax6-dmg-extra",
    "cgb-boot-hwio",
    "mooneye-cgb-extra",
    "mooneye-sgb-boot-regs-extra",
    "samesuite-dmg-extra",
    "samesuite-cgb-extra",
    "magen-cgb-extra",
    "mealybug-tearoom-cgb-extra",
    "little-things-gb-dmg-extra",
    "little-things-gb-cgb-extra",
];
const DOCBOY_CURATED_TEST_ROM_REPORT_SUITE_NAMES: [&str; 4] = [
    "docboy-dmg",
    "docboy-cgb",
    "docboy-cgb-dmg",
    "docboy-cgb-dmg-ext",
];
const GBMICROTEST_CURATED_TEST_ROM_REPORT_SUITE_NAMES: [&str; 1] = ["gbmicrotest"];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedTestRomManifestFile {
    family: Option<String>,
    suite_name: String,
    #[serde(flatten)]
    defaults: CuratedTestRomCaseDefaultsFile,
    #[serde(rename = "case")]
    cases: Vec<CuratedTestRomCaseFile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct CuratedTestRomCaseDefaultsFile {
    source_id: Option<String>,
    source_path: Option<PathBuf>,
    report_console_suffix: Option<bool>,
    report_label: Option<String>,
    timeout_frames: Option<u32>,
    timeout_tcycles: Option<u64>,
    oracle: Option<String>,
    expected: Option<String>,
    fixture: Option<ManifestFixtureField>,
    check_interval_tcycles: Option<u64>,
    check_at_tcycles: Option<u64>,
    memory: Option<Vec<CuratedMemoryByteExpectationFile>>,
    #[serde(rename = "stimulus")]
    stimuli: Option<Vec<CuratedRomStimulusFile>>,
    console: Option<String>,
    revision: Option<String>,
    startup: Option<String>,
    execution_mode: Option<String>,
    stop_condition: Option<String>,
    disabled: Option<bool>,
    comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedTestRomCaseFile {
    family: Option<String>,
    id: String,
    rom: PathBuf,
    source_id: Option<String>,
    source_path: Option<PathBuf>,
    report_console_suffix: Option<bool>,
    report_label: Option<String>,
    timeout_frames: Option<u32>,
    timeout_tcycles: Option<u64>,
    oracle: Option<String>,
    expected: Option<String>,
    fixture: Option<ManifestFixtureField>,
    check_interval_tcycles: Option<u64>,
    check_at_tcycles: Option<u64>,
    memory: Option<Vec<CuratedMemoryByteExpectationFile>>,
    #[serde(rename = "stimulus")]
    stimuli: Option<Vec<CuratedRomStimulusFile>>,
    console: Option<String>,
    revision: Option<String>,
    startup: Option<String>,
    execution_mode: Option<String>,
    stop_condition: Option<String>,
    disabled: Option<bool>,
    comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
struct CuratedMemoryByteExpectationFile {
    address: u16,
    value: u8,
    fail_value: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedRomStimulusFile {
    tcycle: u64,
    button: String,
    pressed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CuratedTestRomManifest {
    suite_family: Option<String>,
    suite_name: String,
    cases: Vec<CuratedTestRomCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CuratedTestRomCase {
    family: String,
    id: String,
    rom: PathBuf,
    source_id: String,
    source_path: PathBuf,
    report_console_suffix: bool,
    report_label: Option<String>,
    timeout: Timeout,
    oracle: String,
    expected: Option<String>,
    fixture: Option<ManifestFixtureField>,
    check_interval_tcycles: Option<u64>,
    check_at_tcycles: Option<u64>,
    memory: Vec<MemoryByteExpectation>,
    stimuli: Vec<ExternalStimulus>,
    console_model: ConsoleModel,
    host_platform: HostPlatform,
    revision: HardwareRevision,
    startup_mode: StartupMode,
    execution_mode: Option<String>,
    stop_condition: Option<String>,
    disabled: bool,
    comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedSuiteStatus {
    suite_name: String,
    family: String,
    cases: Vec<PersistedCaseStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedCaseStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    family: Option<String>,
    rom: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReportCaseMetadata {
    family: String,
    rom: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ReportCaseOrder {
    source_order_missing: bool,
    source_or_manifest_order: usize,
    console_order: usize,
    manifest_order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedSourceManifestFile {
    #[serde(rename = "source")]
    sources: Vec<CuratedSourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedSourceFile {
    id: String,
    #[serde(default, rename = "required_file")]
    required_files: Vec<CuratedRequiredFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedRequiredFile {
    path: PathBuf,
    family: Option<String>,
    rom: Option<PathBuf>,
    target: Option<PathBuf>,
}

pub fn test_rom_store_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(TEST_ROM_STORE_DIR)
}

pub fn test_rom_store_root_for_report(workspace_root: &Path, report_id: &str) -> PathBuf {
    test_rom_store_root(workspace_root).join(report_id)
}

pub fn gb_emulator_shootout_test_rom_store_root(workspace_root: &Path) -> PathBuf {
    test_rom_store_root_for_report(workspace_root, GB_EMULATOR_SHOOTOUT_REPORT_ID)
}

pub fn discover_test_rom_store_root(workspace_root: &Path) -> Option<PathBuf> {
    if let Some(root) = env::var_os(TEST_ROM_ROOT_ENV_VAR) {
        return Some(PathBuf::from(root));
    }

    let default_root = test_rom_store_root(workspace_root);
    default_root.exists().then_some(default_root)
}

pub fn discover_test_rom_store_root_for_report(
    workspace_root: &Path,
    report_id: &str,
) -> Option<PathBuf> {
    if let Some(root) = env::var_os(TEST_ROM_ROOT_ENV_VAR) {
        return Some(PathBuf::from(root).join(report_id));
    }

    let default_root = test_rom_store_root_for_report(workspace_root, report_id);
    default_root.exists().then_some(default_root)
}

pub(crate) fn curated_family_store_prefix(family: &str) -> PathBuf {
    match family {
        "docboy-dmg" => PathBuf::from("dmg"),
        "docboy-cgb" => PathBuf::from("cgb"),
        "docboy-cgb-dmg" => PathBuf::from("cgb-dmg"),
        "docboy-cgb-dmg-ext" => PathBuf::from("cgb-dmg-ext"),
        _ => PathBuf::from(family),
    }
}

pub(crate) fn curated_case_store_relative_path(family: &str, rom: &Path) -> PathBuf {
    curated_family_store_prefix(family).join(rom)
}

fn report_uses_flat_family_store(report_id: Option<&str>, family: &str) -> bool {
    report_id == Some(GBMICROTEST_REPORT_ID) && family == GBMICROTEST_REPORT_ID
}

fn curated_case_store_relative_path_for_report(
    family: &str,
    rom: &Path,
    report_id: Option<&str>,
) -> PathBuf {
    if report_uses_flat_family_store(report_id, family) {
        rom.to_path_buf()
    } else {
        curated_case_store_relative_path(family, rom)
    }
}

pub fn acid_suite() -> RomSuite {
    manifest_suite("acid")
}

pub fn ax6_dmg_extra_suite() -> RomSuite {
    manifest_suite_by_name("ax6-dmg-extra")
}

pub fn samesuite_dmg_extra_suite() -> RomSuite {
    manifest_suite_by_name("samesuite-dmg-extra")
}

pub fn samesuite_cgb_extra_suite() -> RomSuite {
    manifest_suite_by_name("samesuite-cgb-extra")
}

pub fn samesuite_suite() -> RomSuite {
    manifest_suite_by_name("samesuite")
}

pub fn magen_cgb_extra_suite() -> RomSuite {
    manifest_suite_by_name("magen-cgb-extra")
}

pub fn little_things_gb_dmg_extra_suite() -> RomSuite {
    manifest_suite_by_name("little-things-gb-dmg-extra")
}

pub fn little_things_gb_cgb_extra_suite() -> RomSuite {
    manifest_suite_by_name("little-things-gb-cgb-extra")
}

pub fn gbmicrotest_suite() -> RomSuite {
    manifest_suite_by_name("gbmicrotest")
}

pub fn docboy_dmg_suite() -> RomSuite {
    manifest_suite_by_name("docboy-dmg")
}

pub fn docboy_cgb_suite() -> RomSuite {
    manifest_suite_by_name("docboy-cgb")
}

pub fn docboy_cgb_dmg_suite() -> RomSuite {
    manifest_suite_by_name("docboy-cgb-dmg")
}

pub fn docboy_cgb_dmg_ext_suite() -> RomSuite {
    manifest_suite_by_name("docboy-cgb-dmg-ext")
}

pub fn blargg_cpu_instrs_suite() -> RomSuite {
    manifest_suite_by_name("blargg-cpu-instrs")
}

pub fn blargg_dmg_sound_suite() -> RomSuite {
    manifest_suite_by_name("blargg-dmg-sound")
}

pub fn blargg_timing_memory_oam_suite() -> RomSuite {
    manifest_suite_by_name("blargg-timing-memory-oam")
}

pub fn blargg_curated_suites() -> Vec<RomSuite> {
    [
        blargg_cpu_instrs_suite(),
        blargg_dmg_sound_suite(),
        blargg_timing_memory_oam_suite(),
    ]
    .into()
}

pub fn daid_suite() -> RomSuite {
    manifest_suite("daid")
}

pub fn ashiepaws_suite() -> RomSuite {
    manifest_suite("ashiepaws")
}

pub fn cpp_suite() -> RomSuite {
    manifest_suite("cpp")
}

pub fn mealybug_tearoom_suite() -> RomSuite {
    manifest_suite("mealybug-tearoom-tests")
}

pub fn mealybug_tearoom_cgb_extra_suite() -> RomSuite {
    manifest_suite_by_name("mealybug-tearoom-cgb-extra")
}

pub fn mooneye_acceptance_manual_misc_suite() -> RomSuite {
    manifest_suite_by_name("mooneye-acceptance-manual-misc")
}

pub fn mooneye_emulator_mbc1_mbc5_suite() -> RomSuite {
    manifest_suite_by_name("mooneye-emulator-mbc1-mbc5")
}

pub fn mooneye_emulator_mbc2_suite() -> RomSuite {
    manifest_suite_by_name("mooneye-emulator-mbc2")
}

pub fn mooneye_curated_suites() -> Vec<RomSuite> {
    [
        mooneye_acceptance_manual_misc_suite(),
        mooneye_emulator_mbc1_mbc5_suite(),
        mooneye_emulator_mbc2_suite(),
    ]
    .into()
}

pub(crate) fn rom_path_without_store_prefix(rom_path: &Path) -> &Path {
    let mut normalized_path = rom_path;
    if let Ok(stripped) = normalized_path.strip_prefix(TEST_ROM_STORE_DIR) {
        normalized_path = stripped;
    }
    if let Ok(stripped) = normalized_path.strip_prefix(GB_EMULATOR_SHOOTOUT_REPORT_ID) {
        normalized_path = stripped;
    }
    if let Ok(stripped) = normalized_path.strip_prefix(DOCBOY_REPORT_ID) {
        normalized_path = stripped;
    }
    if let Ok(stripped) = normalized_path.strip_prefix(GBMICROTEST_REPORT_ID) {
        normalized_path = stripped;
    }
    normalized_path
}

pub fn curated_test_rom_family_suites() -> Vec<RomSuite> {
    let mut suites = vec![
        acid_suite(),
        blargg_cpu_instrs_suite(),
        blargg_dmg_sound_suite(),
        blargg_timing_memory_oam_suite(),
        cpp_suite(),
        daid_suite(),
        ashiepaws_suite(),
        mealybug_tearoom_suite(),
    ];
    suites.extend(mooneye_curated_suites());
    suites
}

pub fn curated_test_rom_families() -> Vec<String> {
    let families = curated_test_rom_manifests()
        .into_iter()
        .flat_map(|manifest| manifest.cases.into_iter().map(|case| case.family))
        .collect::<BTreeSet<_>>();
    families.into_iter().collect()
}

pub fn curated_test_rom_families_for_report(
    report_id: Option<&str>,
) -> Result<Vec<String>, String> {
    if let Some(report_id) = report_id
        && !matches!(
            report_id,
            DOCBOY_REPORT_ID | GB_EMULATOR_SHOOTOUT_REPORT_ID | GBMICROTEST_REPORT_ID
        )
    {
        return Err(format!("unknown curated test ROM report {report_id:?}"));
    }

    let families = curated_test_rom_manifests()
        .into_iter()
        .filter(|manifest| manifest_matches_optional_report_id(&manifest.suite_name, report_id))
        .flat_map(|manifest| manifest.cases.into_iter().map(|case| case.family))
        .collect::<BTreeSet<_>>();
    Ok(families.into_iter().collect())
}

fn store_root_for_optional_report(workspace_root: &Path, report_id: Option<&str>) -> PathBuf {
    report_id.map_or_else(
        || test_rom_store_root(workspace_root),
        |report_id| test_rom_store_root_for_report(workspace_root, report_id),
    )
}

fn selected_families_for_report<'a>(
    report_id: Option<&str>,
    selected_families: Option<&'a BTreeSet<&'a str>>,
) -> Result<Option<&'a BTreeSet<&'a str>>, String> {
    if let Some(report_id) = report_id
        && !matches!(
            report_id,
            DOCBOY_REPORT_ID | GB_EMULATOR_SHOOTOUT_REPORT_ID | GBMICROTEST_REPORT_ID
        )
    {
        return Err(format!("unknown curated test ROM report {report_id:?}"));
    }

    Ok(selected_families)
}

#[cfg(test)]
pub(crate) fn disabled_curated_rom_paths_for_family(family: &str) -> Vec<PathBuf> {
    curated_test_rom_manifests()
        .into_iter()
        .flat_map(|manifest| manifest.cases.into_iter())
        .filter(|case| case.family == family && case.disabled)
        .map(|case| curated_case_store_relative_path(&case.family, &case.rom))
        .collect()
}

/// Materialize the legacy single-root GBEmulatorShootout subset of the curated ROM store.
///
/// Multi-source rows, such as DocBoy-only fixtures, must be fetched through
/// `fetch_test_roms` or `materialize_curated_test_rom_source_families` so each
/// source is copied from the root that actually owns its manifest paths.
pub fn materialize_curated_test_rom_store(
    workspace_root: &Path,
    gbemu_shootout_root: &Path,
) -> Result<(), String> {
    materialize_curated_test_rom_store_filtered(workspace_root, gbemu_shootout_root, None)?;
    Ok(())
}

/// Materialize selected families from the legacy single-root GBEmulatorShootout source.
///
/// Families that only have non-GBEmulatorShootout rows are rejected here because
/// `gbemu_shootout_root` cannot satisfy their source paths.
pub fn materialize_curated_test_rom_families(
    workspace_root: &Path,
    gbemu_shootout_root: &Path,
    selected_families: &[String],
) -> Result<(), String> {
    let selected_families = selected_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    materialize_curated_test_rom_store_filtered(
        workspace_root,
        gbemu_shootout_root,
        Some(&selected_families),
    )
}

fn materialize_curated_test_rom_store_filtered(
    workspace_root: &Path,
    source_root: &Path,
    selected_families: Option<&BTreeSet<&str>>,
) -> Result<(), String> {
    replace_curated_test_rom_family_roots(
        workspace_root,
        test_rom_store_root(workspace_root),
        Some(GBEMU_SHOOTOUT_SOURCE_ID),
        selected_families,
        None,
    )?;
    materialize_curated_test_rom_source_filtered(
        workspace_root,
        test_rom_store_root(workspace_root),
        Some(GBEMU_SHOOTOUT_SOURCE_ID),
        source_root,
        selected_families,
        None,
    )
}

pub(crate) fn replace_curated_test_rom_families_for_report(
    workspace_root: &Path,
    report_id: Option<&str>,
    selected_families: &[String],
) -> Result<(), String> {
    let selected_families = selected_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    replace_curated_test_rom_family_roots(
        workspace_root,
        store_root_for_optional_report(workspace_root, report_id),
        None,
        selected_families_for_report(report_id, Some(&selected_families))?,
        report_id,
    )
}

fn replace_curated_test_rom_family_roots(
    _workspace_root: &Path,
    store_root: PathBuf,
    source_id: Option<&str>,
    selected_families: Option<&BTreeSet<&str>>,
    report_id: Option<&str>,
) -> Result<(), String> {
    fs::create_dir_all(&store_root).map_err(|error| {
        format!(
            "failed to create curated test ROM store {}: {error}",
            store_root.display()
        )
    })?;

    let selected_cases_by_family =
        curated_test_rom_cases_by_family_from_source(selected_families, source_id, report_id);
    let mut materialized_families = BTreeSet::new();
    for family in selected_cases_by_family.keys() {
        materialized_families.insert(family.clone());
        let cases = selected_cases_by_family
            .get(family)
            .expect("selected family key should have cases");
        if report_uses_flat_family_store(report_id, family) {
            replace_curated_flat_family_roots(&store_root, cases, report_id)?;
        } else {
            let family_root = store_root.join(curated_family_store_prefix(family));
            if family_root.exists() {
                fs::remove_dir_all(&family_root).map_err(|error| {
                    format!(
                        "failed to replace curated family directory {}: {error}",
                        family_root.display()
                    )
                })?;
            }
            fs::create_dir_all(&family_root).map_err(|error| {
                format!(
                    "failed to create curated family directory {}: {error}",
                    family_root.display()
                )
            })?;
        }
    }

    if let Some(selected_families) = selected_families {
        let unknown_families = selected_families
            .iter()
            .filter(|family| !materialized_families.contains(**family))
            .copied()
            .collect::<Vec<_>>();
        if !unknown_families.is_empty() {
            return Err(format!(
                "unknown curated test ROM family selection: {}",
                unknown_families.join(", ")
            ));
        }
    }

    Ok(())
}

fn replace_curated_flat_family_roots(
    store_root: &Path,
    cases: &BTreeMap<PathBuf, CuratedTestRomCase>,
    report_id: Option<&str>,
) -> Result<(), String> {
    let mut roots = BTreeSet::new();
    for case in cases.values() {
        let relative_path =
            curated_case_store_relative_path_for_report(&case.family, &case.rom, report_id);
        let Some(root) = relative_path.components().next() else {
            continue;
        };
        roots.insert(PathBuf::from(root.as_os_str()));
    }

    for root in roots {
        let root_path = store_root.join(&root);
        if !root_path.exists() {
            continue;
        }
        if root_path.is_dir() {
            fs::remove_dir_all(&root_path).map_err(|error| {
                format!(
                    "failed to replace curated flat family directory {}: {error}",
                    root_path.display()
                )
            })?;
        } else {
            fs::remove_file(&root_path).map_err(|error| {
                format!(
                    "failed to replace curated flat family file {}: {error}",
                    root_path.display()
                )
            })?;
        }
    }

    Ok(())
}

pub(crate) fn materialize_curated_test_rom_source_report_families(
    workspace_root: &Path,
    report_id: Option<&str>,
    source_id: &str,
    source_root: &Path,
    selected_families: &[String],
) -> Result<(), String> {
    let selected_families = selected_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let selected_families = selected_families_for_report(report_id, Some(&selected_families))?;
    materialize_curated_test_rom_source_filtered(
        workspace_root,
        store_root_for_optional_report(workspace_root, report_id),
        Some(source_id),
        source_root,
        selected_families,
        report_id,
    )
}

fn materialize_curated_test_rom_source_filtered(
    _workspace_root: &Path,
    store_root: PathBuf,
    source_id: Option<&str>,
    source_root: &Path,
    selected_families: Option<&BTreeSet<&str>>,
    report_id: Option<&str>,
) -> Result<(), String> {
    let mut copied_targets = BTreeSet::new();
    for (_, cases) in
        curated_test_rom_cases_by_family_from_source(selected_families, source_id, report_id)
    {
        for case in cases.into_values() {
            let target_path = store_root.join(curated_case_store_relative_path_for_report(
                &case.family,
                &case.rom,
                report_id,
            ));
            copy_curated_source_rom(source_root, &case.source_path, &target_path)?;
            copied_targets.insert(target_path);
        }
    }
    if let Some(source_id) = source_id {
        for (family, source_path, target) in
            curated_explicit_required_files_for_source(source_id, selected_families, report_id)
        {
            let target_path = store_root.join(curated_case_store_relative_path_for_report(
                &family, &target, report_id,
            ));
            if copied_targets.insert(target_path.clone()) {
                copy_curated_source_rom(source_root, &source_path, &target_path)?;
            }
        }
    }
    Ok(())
}

fn curated_explicit_required_files_for_source(
    source_id: &str,
    selected_families: Option<&BTreeSet<&str>>,
    report_id: Option<&str>,
) -> Vec<(String, PathBuf, PathBuf)> {
    let parsed: CuratedSourceManifestFile = toml::from_str(curated_source_manifest_text(report_id))
        .unwrap_or_else(|error| panic!("failed to parse curated source manifest: {error}"));
    parsed
        .sources
        .into_iter()
        .filter(|source| source.id == source_id)
        .flat_map(|source| source.required_files)
        .filter_map(|file| {
            let path = file.path;
            let family = file
                .family
                .or_else(|| required_file_family(&path).map(str::to_string))?;
            if let Some(selected_families) = selected_families
                && !selected_families.contains(family.as_str())
            {
                return None;
            }
            let target = file.target.or(file.rom)?;
            Some((family, path, target))
        })
        .collect()
}

fn required_file_family(path: &Path) -> Option<&str> {
    let mut components = path.components();
    if components.next()?.as_os_str() != "testroms" {
        return None;
    }
    components.next()?.as_os_str().to_str()
}

fn curated_source_manifest_text(report_id: Option<&str>) -> &'static str {
    match report_id {
        Some(DOCBOY_REPORT_ID) => include_str!("../data/docboy/sources.toml"),
        Some(GBMICROTEST_REPORT_ID) => include_str!("../data/gbmicrotest/sources.toml"),
        Some(GB_EMULATOR_SHOOTOUT_REPORT_ID) => {
            include_str!("../data/gb-emulator-shootout/sources.toml")
        }
        Some(report_id) => panic!("unknown curated test ROM report {report_id:?}"),
        None => include_str!("../data/sources.toml"),
    }
}

fn curated_test_rom_cases_by_family_from_source(
    selected_families: Option<&BTreeSet<&str>>,
    source_id: Option<&str>,
    report_id: Option<&str>,
) -> BTreeMap<String, BTreeMap<PathBuf, CuratedTestRomCase>> {
    let mut cases_by_family = BTreeMap::<String, BTreeMap<PathBuf, CuratedTestRomCase>>::new();
    for manifest in curated_test_rom_manifests() {
        if !manifest_matches_optional_report_id(&manifest.suite_name, report_id) {
            continue;
        }
        for case in manifest.cases {
            if let Some(source_id) = source_id
                && case.source_id != source_id
            {
                continue;
            }
            if let Some(selected_families) = selected_families
                && !selected_families.contains(case.family.as_str())
            {
                continue;
            }
            cases_by_family
                .entry(case.family.clone())
                .or_default()
                .entry(case.rom.clone())
                .or_insert(case);
        }
    }
    cases_by_family
}

#[cfg(test)]
fn copy_curated_rom(
    gbemu_shootout_root: &Path,
    family: &str,
    rom: &Path,
    target_path: &Path,
) -> Result<(), String> {
    let source_path = PathBuf::from(GBEMU_SHOOTOUT_TESTROMS_DIR)
        .join(family)
        .join(rom);
    copy_curated_source_rom(gbemu_shootout_root, &source_path, target_path)
}

fn copy_curated_source_rom(
    source_root: &Path,
    source_path: &Path,
    target_path: &Path,
) -> Result<(), String> {
    let source_path = source_root.join(source_path);
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create curated ROM parent {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::copy(&source_path, target_path).map_err(|error| {
        format!(
            "failed to copy curated ROM {} -> {}: {error}",
            source_path.display(),
            target_path.display()
        )
    })?;

    Ok(())
}

pub fn update_curated_test_report(
    workspace_root: &Path,
    report: &RomSuiteReport,
) -> Result<Option<PathBuf>, String> {
    let Some(family) = &report.family else {
        return Ok(None);
    };

    let report_id = suite_report_id(&report.suite_name);
    let store_root = store_root_for_optional_report(workspace_root, report_id);
    fs::create_dir_all(&store_root).map_err(|error| {
        format!(
            "failed to create curated test ROM store {}: {error}",
            store_root.display()
        )
    })?;

    let status_root = store_root.join(TEST_ROM_STATUS_DIR_NAME);
    fs::create_dir_all(&status_root).map_err(|error| {
        format!(
            "failed to create curated test ROM status root {}: {error}",
            status_root.display()
        )
    })?;

    let suite_status_path = status_root.join(format!(
        "{}.toml",
        suite_status_file_stem(&report.suite_name)
    ));
    let mut merged_case_statuses = Vec::new();
    if let Some(persisted) = load_persisted_suite_status(&suite_status_path)?.filter(|persisted| {
        persisted.suite_name == report.suite_name && persisted.family == *family
    }) {
        merge_persisted_case_statuses(&mut merged_case_statuses, family, persisted.cases);
    }

    for case in &report.cases {
        let metadata = report_case_metadata(&report.suite_name, family, case);
        let status = case.outcome.report_status().to_string();

        let persisted_status = PersistedCaseStatus {
            family: (metadata.family != *family).then_some(metadata.family),
            rom: metadata.rom,
            status,
        };
        merged_case_statuses
            .retain(|entry| !persisted_case_statuses_match(family, entry, &persisted_status));
        merged_case_statuses.push(persisted_status);
    }
    sort_persisted_case_statuses(&report.suite_name, family, &mut merged_case_statuses);

    let persisted = PersistedSuiteStatus {
        suite_name: report.suite_name.clone(),
        family: family.clone(),
        cases: merged_case_statuses,
    };
    let persisted_text = toml::to_string(&persisted).map_err(|error| {
        format!(
            "failed to serialize curated test ROM status for suite {}: {error}",
            report.suite_name
        )
    })?;
    fs::write(&suite_status_path, persisted_text).map_err(|error| {
        format!(
            "failed to write curated test ROM status {}: {error}",
            suite_status_path.display()
        )
    })?;

    let mut suites = Vec::new();
    for entry in fs::read_dir(&status_root).map_err(|error| {
        format!(
            "failed to read curated test ROM status directory {}: {error}",
            status_root.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read curated test ROM status entry in {}: {error}",
                status_root.display()
            )
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|error| {
            format!(
                "failed to read curated test ROM status {}: {error}",
                path.display()
            )
        })?;
        let persisted: PersistedSuiteStatus = toml::from_str(&text).map_err(|error| {
            format!(
                "failed to parse curated test ROM status {}: {error}",
                path.display()
            )
        })?;
        if !status_matches_report_id(&persisted.suite_name, report_id) {
            continue;
        }
        if !suite_status_path_matches_suite(&path, &persisted.suite_name) {
            continue;
        }
        push_or_merge_persisted_suite_status(
            &mut suites,
            normalize_persisted_suite_status(persisted),
        );
    }

    let standard_suites = report_suites_for_kind(&suites, CuratedTestReportKind::Standard);
    let extra_suites = report_suites_for_kind(&suites, CuratedTestReportKind::Extra);
    let docboy_suites = report_suites_for_kind(&suites, CuratedTestReportKind::DocBoy);
    let gbmicrotest_suites = report_suites_for_kind(&suites, CuratedTestReportKind::Gbmicrotest);
    let standard_report_path = if report_id == Some(GB_EMULATOR_SHOOTOUT_REPORT_ID) {
        write_markdown_report_file_if_needed(
            &store_root,
            TEST_ROM_REPORT_FILE_NAME,
            &standard_suites,
            CuratedTestReportKind::Standard,
        )?
    } else {
        remove_markdown_report_file_if_present(&store_root.join(TEST_ROM_REPORT_FILE_NAME))?;
        None
    };
    let extra_report_path = if report_id.is_none() {
        write_markdown_report_file_if_needed(
            &store_root,
            TEST_ROM_EXTRA_REPORT_FILE_NAME,
            &extra_suites,
            CuratedTestReportKind::Extra,
        )?
    } else {
        remove_markdown_report_file_if_present(&store_root.join(TEST_ROM_EXTRA_REPORT_FILE_NAME))?;
        None
    };
    let docboy_report_path = if report_id == Some(DOCBOY_REPORT_ID) {
        write_markdown_report_file_if_needed(
            &store_root,
            TEST_ROM_DOCBOY_REPORT_FILE_NAME,
            &docboy_suites,
            CuratedTestReportKind::DocBoy,
        )?
    } else {
        None
    };
    let gbmicrotest_report_path = if report_id == Some(GBMICROTEST_REPORT_ID) {
        write_markdown_report_file_if_needed(
            &store_root,
            TEST_ROM_GBMICROTEST_REPORT_FILE_NAME,
            &gbmicrotest_suites,
            CuratedTestReportKind::Gbmicrotest,
        )?
    } else {
        None
    };

    let report_path = if suite_uses_gbmicrotest_test_report(&report.suite_name) {
        gbmicrotest_report_path
    } else if suite_uses_docboy_test_report(&report.suite_name) {
        docboy_report_path
    } else if suite_uses_extra_test_report(&report.suite_name) {
        extra_report_path
    } else {
        standard_report_path
    };

    Ok(report_path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CuratedTestReportKind {
    Standard,
    Extra,
    DocBoy,
    Gbmicrotest,
}

fn report_suites_for_kind(
    suites: &[PersistedSuiteStatus],
    report_kind: CuratedTestReportKind,
) -> Vec<PersistedSuiteStatus> {
    suites
        .iter()
        .filter(|suite| suite_test_report_kind(&suite.suite_name) == report_kind)
        .cloned()
        .collect()
}

fn suite_uses_extra_test_report(suite_name: &str) -> bool {
    EXTRA_CURATED_TEST_ROM_REPORT_SUITE_NAMES.contains(&suite_name)
}

fn suite_uses_docboy_test_report(suite_name: &str) -> bool {
    DOCBOY_CURATED_TEST_ROM_REPORT_SUITE_NAMES.contains(&suite_name)
}

fn suite_uses_gbmicrotest_test_report(suite_name: &str) -> bool {
    GBMICROTEST_CURATED_TEST_ROM_REPORT_SUITE_NAMES.contains(&suite_name)
}

fn suite_test_report_kind(suite_name: &str) -> CuratedTestReportKind {
    if suite_uses_gbmicrotest_test_report(suite_name) {
        CuratedTestReportKind::Gbmicrotest
    } else if suite_uses_docboy_test_report(suite_name) {
        CuratedTestReportKind::DocBoy
    } else if suite_uses_extra_test_report(suite_name) {
        CuratedTestReportKind::Extra
    } else {
        CuratedTestReportKind::Standard
    }
}

fn suite_report_id(suite_name: &str) -> Option<&'static str> {
    match suite_test_report_kind(suite_name) {
        CuratedTestReportKind::Standard => Some(GB_EMULATOR_SHOOTOUT_REPORT_ID),
        CuratedTestReportKind::DocBoy => Some(DOCBOY_REPORT_ID),
        CuratedTestReportKind::Gbmicrotest => Some(GBMICROTEST_REPORT_ID),
        CuratedTestReportKind::Extra => None,
    }
}

fn manifest_matches_optional_report_id(suite_name: &str, report_id: Option<&str>) -> bool {
    match report_id {
        Some(report_id) => suite_report_id(suite_name) == Some(report_id),
        None => suite_report_id(suite_name).is_none(),
    }
}

fn status_matches_report_id(suite_name: &str, report_id: Option<&str>) -> bool {
    suite_report_id(suite_name) == report_id
}

fn report_family_order_for_kind(report_kind: CuratedTestReportKind) -> &'static [&'static str] {
    match report_kind {
        CuratedTestReportKind::Standard => &CURATED_TEST_ROM_REPORT_FAMILY_ORDER,
        CuratedTestReportKind::Extra => &CURATED_TEST_ROM_EXTRA_REPORT_FAMILY_ORDER,
        CuratedTestReportKind::DocBoy => &CURATED_TEST_ROM_DOCBOY_REPORT_FAMILY_ORDER,
        CuratedTestReportKind::Gbmicrotest => &CURATED_TEST_ROM_GBMICROTEST_REPORT_FAMILY_ORDER,
    }
}

fn write_markdown_report_file(
    store_root: &Path,
    file_name: &str,
    suites: &[PersistedSuiteStatus],
    report_kind: CuratedTestReportKind,
) -> Result<PathBuf, String> {
    let report_path = store_root.join(file_name);
    fs::write(
        &report_path,
        render_markdown_report_for_kind(suites, report_kind),
    )
    .map_err(|error| {
        format!(
            "failed to write curated test ROM report {}: {error}",
            report_path.display()
        )
    })?;

    Ok(report_path)
}

fn write_markdown_report_file_if_needed(
    store_root: &Path,
    file_name: &str,
    suites: &[PersistedSuiteStatus],
    report_kind: CuratedTestReportKind,
) -> Result<Option<PathBuf>, String> {
    if suites.is_empty() {
        let report_path = store_root.join(file_name);
        remove_markdown_report_file_if_present(&report_path)?;
        return Ok(None);
    }

    write_markdown_report_file(store_root, file_name, suites, report_kind).map(Some)
}

fn remove_markdown_report_file_if_present(report_path: &Path) -> Result<(), String> {
    match fs::remove_file(report_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove empty curated test ROM report {}: {error}",
            report_path.display()
        )),
    }
}

fn suite_status_file_stem(suite_name: &str) -> &str {
    match suite_name {
        "docboy-dmg" => "docboy-dmg",
        "docboy-cgb" => "docboy-cgb",
        "docboy-cgb-dmg" => "docboy-cgb-dmg",
        "docboy-cgb-dmg-ext" => "docboy-cgb-dmg-ext",
        _ => suite_name,
    }
}

fn suite_status_path_matches_suite(path: &Path, suite_name: &str) -> bool {
    path.file_stem().and_then(|stem| stem.to_str()) == Some(suite_status_file_stem(suite_name))
}

fn merge_persisted_case_statuses(
    target: &mut Vec<PersistedCaseStatus>,
    default_family: &str,
    cases: Vec<PersistedCaseStatus>,
) {
    for case in cases {
        target.retain(|existing| !persisted_case_statuses_match(default_family, existing, &case));
        target.push(case);
    }
}

fn persisted_case_statuses_match(
    default_family: &str,
    left: &PersistedCaseStatus,
    right: &PersistedCaseStatus,
) -> bool {
    left.family.as_deref().unwrap_or(default_family)
        == right.family.as_deref().unwrap_or(default_family)
        && left.rom == right.rom
}

fn push_or_merge_persisted_suite_status(
    suites: &mut Vec<PersistedSuiteStatus>,
    suite: PersistedSuiteStatus,
) {
    if let Some(existing) = suites
        .iter_mut()
        .find(|existing| existing.suite_name == suite.suite_name && existing.family == suite.family)
    {
        merge_persisted_case_statuses(&mut existing.cases, &suite.family, suite.cases);
        sort_persisted_case_statuses(&suite.suite_name, &suite.family, &mut existing.cases);
    } else {
        suites.push(suite);
    }
}

fn load_persisted_suite_status(path: &Path) -> Result<Option<PersistedSuiteStatus>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read curated test ROM status {}: {error}",
                path.display()
            ));
        }
    };

    let persisted = toml::from_str(&text).map_err(|error| {
        format!(
            "failed to parse curated test ROM status {}: {error}",
            path.display()
        )
    })?;
    Ok(Some(normalize_persisted_suite_status(persisted)))
}

fn normalize_persisted_suite_status(mut persisted: PersistedSuiteStatus) -> PersistedSuiteStatus {
    let suite_has_manifest = curated_manifest_for_suite(&persisted.suite_name).is_some();
    let mut normalized_cases = Vec::with_capacity(persisted.cases.len());
    for mut case in persisted.cases {
        let family = case.family.as_deref().unwrap_or(&persisted.family);
        if let Some(metadata) = manifest_report_metadata_for_persisted_suite_case(
            &persisted.suite_name,
            family,
            &case.rom,
        ) {
            case.family = (metadata.family != persisted.family).then_some(metadata.family);
            case.rom = metadata.rom;
            normalized_cases.push(case);
        } else if !suite_has_manifest {
            if let Some(metadata) =
                manifest_report_metadata_for_any_persisted_case(family, &case.rom)
            {
                case.family = (metadata.family != persisted.family).then_some(metadata.family);
                case.rom = metadata.rom;
            }
            normalized_cases.push(case);
        }
    }
    persisted.cases = normalized_cases;
    sort_persisted_case_statuses(
        &persisted.suite_name,
        &persisted.family,
        &mut persisted.cases,
    );
    persisted
}

fn curated_manifest_for_suite(suite_name: &str) -> Option<&'static CuratedTestRomManifest> {
    curated_test_rom_manifest_catalog()
        .iter()
        .find(|manifest| manifest.suite_name == suite_name)
}

fn manifest_report_metadata_for_persisted_suite_case(
    suite_name: &str,
    family: &str,
    rom: &str,
) -> Option<ReportCaseMetadata> {
    curated_test_rom_manifest_catalog()
        .iter()
        .filter(|manifest| manifest.suite_name == suite_name)
        .flat_map(|manifest| manifest.cases.iter())
        .filter(|case| !case.disabled)
        .find(|case| persisted_case_matches_manifest_case(family, rom, case))
        .map(manifest_case_report_metadata)
}

fn manifest_report_metadata_for_any_persisted_case(
    family: &str,
    rom: &str,
) -> Option<ReportCaseMetadata> {
    curated_test_rom_manifest_catalog()
        .iter()
        .flat_map(|manifest| manifest.cases.iter())
        .filter(|case| !case.disabled)
        .find(|case| persisted_case_matches_manifest_case(family, rom, case))
        .map(manifest_case_report_metadata)
}

fn manifest_case_report_metadata(case: &CuratedTestRomCase) -> ReportCaseMetadata {
    ReportCaseMetadata {
        family: case.family.clone(),
        rom: manifest_case_report_rom_display(case),
    }
}

fn persisted_case_matches_manifest_case(
    family: &str,
    rom: &str,
    case: &CuratedTestRomCase,
) -> bool {
    let rom_path = curated_case_store_relative_path(&case.family, &case.rom);
    let family_matches = family == case.family;
    let display_matches = report_rom_display(&case.family, &rom_path) == rom
        || manifest_case_report_rom_display(case) == rom;
    let full_path_matches = rom_path.to_string_lossy() == rom;

    family_matches && (display_matches || full_path_matches)
}

fn manifest_case_order(suite_name: &str, family: &str, rom: &str) -> Option<ReportCaseOrder> {
    manifest_case_order_for_suite(suite_name, family, rom)
        .or_else(|| manifest_case_order_for_any_suite(family, rom))
}

fn manifest_case_order_for_suite(
    suite_name: &str,
    family: &str,
    rom: &str,
) -> Option<ReportCaseOrder> {
    let suite_manifest = curated_test_rom_manifest_catalog()
        .iter()
        .find(|manifest| manifest.suite_name == suite_name)?;
    for (case_manifest_order, case) in suite_manifest
        .cases
        .iter()
        .filter(|case| case.family == family && !case.disabled)
        .enumerate()
    {
        if persisted_case_matches_manifest_case(family, rom, case) {
            let source_order = curated_source_rom_order(&case.family, &case.rom);
            return Some(ReportCaseOrder {
                source_order_missing: source_order.is_none(),
                source_or_manifest_order: source_order.unwrap_or(case_manifest_order),
                console_order: console_report_order(case.console_model, case.host_platform),
                manifest_order: case_manifest_order,
            });
        }
    }

    None
}

fn manifest_case_order_for_any_suite(family: &str, rom: &str) -> Option<ReportCaseOrder> {
    for (case_manifest_order, case) in curated_test_rom_manifest_catalog()
        .iter()
        .flat_map(|manifest| manifest.cases.iter())
        .filter(|case| case.family == family && !case.disabled)
        .enumerate()
    {
        if persisted_case_matches_manifest_case(family, rom, case) {
            let source_order = curated_source_rom_order(&case.family, &case.rom);
            return Some(ReportCaseOrder {
                source_order_missing: source_order.is_none(),
                source_or_manifest_order: source_order.unwrap_or(case_manifest_order),
                console_order: console_report_order(case.console_model, case.host_platform),
                manifest_order: case_manifest_order,
            });
        }
    }

    None
}

fn sort_persisted_case_statuses(
    suite_name: &str,
    family: &str,
    case_statuses: &mut [PersistedCaseStatus],
) {
    let family_order = report_family_order_for_kind(suite_test_report_kind(suite_name));
    case_statuses.sort_by_cached_key(|entry| {
        let entry_family = entry.family.as_deref().unwrap_or(family);
        let rank = report_family_rank(entry_family, family_order);
        let order = manifest_case_order(suite_name, entry_family, &entry.rom);
        (
            rank.is_none(),
            rank.unwrap_or(usize::MAX),
            entry_family.to_string(),
            order.is_none(),
            order.unwrap_or(ReportCaseOrder::fallback()),
            entry.rom.clone(),
        )
    });
}

fn manifest_suite(family: &str) -> RomSuite {
    let manifest = curated_test_rom_manifests()
        .into_iter()
        .find(|manifest| manifest.suite_family.as_deref() == Some(family))
        .unwrap_or_else(|| panic!("missing curated test ROM manifest for family {family}"));

    let report_id = suite_report_id(&manifest.suite_name);
    let mut suite = RomSuite::new(manifest.suite_name).with_family(family);
    if let Some(report_id) = report_id {
        suite = suite.with_report_id(report_id);
    }
    for case in manifest.cases {
        if case.disabled {
            continue;
        }
        let mut rom_case = manifest_case_to_rom_test_case(case, report_id);
        if let Some(report_id) = report_id {
            rom_case = rom_case.with_report_id(report_id);
        }
        suite.push_case(rom_case);
    }
    suite
}

pub fn cgb_boot_hwio_suite() -> RomSuite {
    manifest_suite_by_name("cgb-boot-hwio")
}

pub fn mooneye_cgb_extra_suite() -> RomSuite {
    manifest_suite_by_name("mooneye-cgb-extra")
}

pub fn mooneye_sgb_boot_regs_extra_suite() -> RomSuite {
    manifest_suite_by_name("mooneye-sgb-boot-regs-extra")
}

pub fn blargg_cgb_sound_suite() -> RomSuite {
    manifest_suite_by_name("blargg-cgb-sound")
}

pub fn samesuite_apu_suite() -> RomSuite {
    manifest_suite_by_name("samesuite-apu")
}

pub fn ax6_suite() -> RomSuite {
    manifest_suite_by_name("ax6")
}

fn manifest_suite_by_name(suite_name: &str) -> RomSuite {
    let manifest = curated_test_rom_manifests()
        .into_iter()
        .find(|manifest| manifest.suite_name == suite_name)
        .unwrap_or_else(|| panic!("missing curated test ROM manifest for suite {suite_name}"));
    let suite_family = manifest
        .suite_family
        .as_deref()
        .unwrap_or(&manifest.suite_name)
        .to_string();

    let report_id = suite_report_id(&manifest.suite_name);
    let mut suite = RomSuite::new(manifest.suite_name).with_family(suite_family);
    if let Some(report_id) = report_id {
        suite = suite.with_report_id(report_id);
    }
    for case in manifest.cases {
        if case.disabled {
            continue;
        }
        let mut rom_case = manifest_case_to_rom_test_case(case, report_id);
        if let Some(report_id) = report_id {
            rom_case = rom_case.with_report_id(report_id);
        }
        suite.push_case(rom_case);
    }
    suite
}

fn curated_test_rom_manifests() -> Vec<CuratedTestRomManifest> {
    curated_test_rom_manifest_catalog().to_vec()
}

fn curated_test_rom_manifest_catalog() -> &'static [CuratedTestRomManifest] {
    CURATED_TEST_ROM_MANIFEST_CACHE
        .get_or_init(parse_curated_test_rom_manifests)
        .as_slice()
}

fn parse_curated_test_rom_manifests() -> Vec<CuratedTestRomManifest> {
    curated_test_rom_manifest_texts()
        .into_iter()
        .map(|(source_path, source_text)| parse_manifest(source_path, source_text))
        .collect()
}

fn curated_test_rom_manifest_texts() -> [(&'static str, &'static str); 30] {
    [
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/acid.toml",
            include_str!("../data/gb-emulator-shootout/acid.toml"),
        ),
        (
            "crates/gb-test-runner/data/ax6.toml",
            include_str!("../data/ax6.toml"),
        ),
        (
            "crates/gb-test-runner/data/samesuite.toml",
            include_str!("../data/samesuite.toml"),
        ),
        (
            "crates/gb-test-runner/data/samesuite-cgb.toml",
            include_str!("../data/samesuite-cgb.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/samesuite.toml",
            include_str!("../data/gb-emulator-shootout/samesuite.toml"),
        ),
        (
            "crates/gb-test-runner/data/magen.toml",
            include_str!("../data/magen.toml"),
        ),
        (
            "crates/gb-test-runner/data/little-things-gb.toml",
            include_str!("../data/little-things-gb.toml"),
        ),
        (
            "crates/gb-test-runner/data/little-things-gb-cgb.toml",
            include_str!("../data/little-things-gb-cgb.toml"),
        ),
        (
            "crates/gb-test-runner/data/gbmicrotest/gbmicrotest.toml",
            include_str!("../data/gbmicrotest/gbmicrotest.toml"),
        ),
        (
            "crates/gb-test-runner/data/docboy/docboy-dmg.toml",
            include_str!("../data/docboy/docboy-dmg.toml"),
        ),
        (
            "crates/gb-test-runner/data/docboy/docboy-cgb.toml",
            include_str!("../data/docboy/docboy-cgb.toml"),
        ),
        (
            "crates/gb-test-runner/data/docboy/docboy-cgb-dmg.toml",
            include_str!("../data/docboy/docboy-cgb-dmg.toml"),
        ),
        (
            "crates/gb-test-runner/data/docboy/docboy-cgb-dmg-ext.toml",
            include_str!("../data/docboy/docboy-cgb-dmg-ext.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/blargg-cgb-sound.toml",
            include_str!("../data/gb-emulator-shootout/blargg-cgb-sound.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/samesuite-apu.toml",
            include_str!("../data/gb-emulator-shootout/samesuite-apu.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-boot-hwio.toml",
            include_str!("../data/cgb-boot-hwio.toml"),
        ),
        (
            "crates/gb-test-runner/data/mooneye-sgb-boot-regs.toml",
            include_str!("../data/mooneye-sgb-boot-regs.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/ax6.toml",
            include_str!("../data/gb-emulator-shootout/ax6.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/blargg-cpu-instrs.toml",
            include_str!("../data/gb-emulator-shootout/blargg-cpu-instrs.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/blargg-dmg-sound.toml",
            include_str!("../data/gb-emulator-shootout/blargg-dmg-sound.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/blargg-timing-memory-oam.toml",
            include_str!("../data/gb-emulator-shootout/blargg-timing-memory-oam.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/daid.toml",
            include_str!("../data/gb-emulator-shootout/daid.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/cpp.toml",
            include_str!("../data/gb-emulator-shootout/cpp.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/ashiepaws.toml",
            include_str!("../data/gb-emulator-shootout/ashiepaws.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/mealybug-tearoom-tests.toml",
            include_str!("../data/gb-emulator-shootout/mealybug-tearoom-tests.toml"),
        ),
        (
            "crates/gb-test-runner/data/mealybug-tearoom-tests-cgb.toml",
            include_str!("../data/mealybug-tearoom-tests-cgb.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/mooneye-acceptance-manual-misc.toml",
            include_str!("../data/gb-emulator-shootout/mooneye-acceptance-manual-misc.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/mooneye-emulator-mbc1-mbc5.toml",
            include_str!("../data/gb-emulator-shootout/mooneye-emulator-mbc1-mbc5.toml"),
        ),
        (
            "crates/gb-test-runner/data/gb-emulator-shootout/mooneye-emulator-mbc2.toml",
            include_str!("../data/gb-emulator-shootout/mooneye-emulator-mbc2.toml"),
        ),
        (
            "crates/gb-test-runner/data/mooneye-cgb.toml",
            include_str!("../data/mooneye-cgb.toml"),
        ),
    ]
}

fn parse_manifest(source_path: &'static str, source_text: &'static str) -> CuratedTestRomManifest {
    let parsed: CuratedTestRomManifestFile = toml::from_str(source_text)
        .unwrap_or_else(|error| panic!("failed to parse {source_path}: {error}"));
    let CuratedTestRomManifestFile {
        family,
        suite_name,
        defaults,
        cases,
    } = parsed;
    let report_id = suite_report_id(&suite_name);

    CuratedTestRomManifest {
        suite_family: family.clone(),
        suite_name,
        cases: cases
            .into_iter()
            .map(|case| {
                parse_manifest_case(source_path, family.as_deref(), report_id, &defaults, case)
            })
            .collect(),
    }
}

fn parse_manifest_case(
    source_path: &str,
    manifest_family: Option<&str>,
    report_id: Option<&str>,
    defaults: &CuratedTestRomCaseDefaultsFile,
    case: CuratedTestRomCaseFile,
) -> CuratedTestRomCase {
    let manifest_path = source_path;
    let family = case
        .family
        .or_else(|| manifest_family.map(str::to_string))
        .unwrap_or_else(|| {
            panic!(
                "missing family for curated case {} in {source_path}",
                case.id
            )
        });
    let (console_model, host_platform) = parse_manifest_console_profile(
        source_path,
        &case.id,
        case.console
            .as_deref()
            .or(defaults.console.as_deref())
            .unwrap_or("dmg"),
    );
    let revision = case
        .revision
        .as_deref()
        .or(defaults.revision.as_deref())
        .map(|revision| parse_manifest_revision(source_path, &case.id, revision))
        .unwrap_or_else(|| console_model.default_revision());
    if !console_model.supports_revision(revision) {
        panic!(
            "curated case {} in {source_path} uses revision {:?} with unsupported console {:?}",
            case.id, revision, console_model
        );
    }
    let startup_mode = parse_manifest_startup_mode(
        source_path,
        &case.id,
        case.startup
            .as_deref()
            .or(defaults.startup.as_deref())
            .unwrap_or("skip-boot"),
    );
    let default_source_id = defaults.source_id.clone();
    let default_source_path = defaults.source_path.clone();
    let (report_source_id, report_source_path) = if (case.source_id.is_none()
        && default_source_id.is_none())
        || (case.source_path.is_none() && default_source_path.is_none())
    {
        resolve_report_local_source_for_case(report_id, &family, &case.rom)
    } else {
        (None, None)
    };
    let source_id = case
        .source_id
        .or(default_source_id)
        .or(report_source_id)
        .unwrap_or_else(|| GBEMU_SHOOTOUT_SOURCE_ID.to_string());
    let source_path = case
        .source_path
        .or(default_source_path)
        .or(report_source_path)
        .unwrap_or_else(|| {
            PathBuf::from(GBEMU_SHOOTOUT_TESTROMS_DIR)
                .join(&family)
                .join(&case.rom)
        });
    let (timeout_frames, timeout_tcycles) =
        if case.timeout_frames.is_some() || case.timeout_tcycles.is_some() {
            (case.timeout_frames, case.timeout_tcycles)
        } else {
            (defaults.timeout_frames, defaults.timeout_tcycles)
        };
    let timeout = parse_manifest_timeout(&source_path, timeout_frames, timeout_tcycles);
    let case_id = case.id;
    let oracle = case
        .oracle
        .or_else(|| defaults.oracle.clone())
        .unwrap_or_else(|| panic!("missing oracle for curated case {case_id} in {manifest_path}"));
    let disabled = case.disabled.or(defaults.disabled).unwrap_or(false);
    let comment = normalize_manifest_case_comment(
        Path::new(manifest_path),
        &case_id,
        disabled,
        case.comment.or_else(|| defaults.comment.clone()),
    );

    CuratedTestRomCase {
        family,
        id: case_id.clone(),
        rom: case.rom,
        source_id,
        source_path: source_path.clone(),
        report_console_suffix: case
            .report_console_suffix
            .or(defaults.report_console_suffix)
            .unwrap_or(false),
        report_label: case.report_label.or_else(|| defaults.report_label.clone()),
        timeout,
        oracle,
        expected: case.expected.or_else(|| defaults.expected.clone()),
        fixture: case.fixture.or_else(|| defaults.fixture.clone()),
        check_interval_tcycles: case
            .check_interval_tcycles
            .or(defaults.check_interval_tcycles),
        check_at_tcycles: case.check_at_tcycles.or(defaults.check_at_tcycles),
        memory: parse_manifest_memory(
            case.memory
                .or_else(|| defaults.memory.clone())
                .unwrap_or_default(),
        ),
        stimuli: case
            .stimuli
            .or_else(|| defaults.stimuli.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|stimulus| parse_manifest_stimulus(&source_path, &case_id, stimulus))
            .collect(),
        console_model,
        host_platform,
        revision,
        startup_mode,
        execution_mode: case
            .execution_mode
            .or_else(|| defaults.execution_mode.clone()),
        stop_condition: case
            .stop_condition
            .or_else(|| defaults.stop_condition.clone()),
        disabled,
        comment,
    }
}

fn resolve_report_local_source_for_case(
    report_id: Option<&str>,
    family: &str,
    rom: &Path,
) -> (Option<String>, Option<PathBuf>) {
    let Some(report_id) = report_id else {
        return (None, None);
    };

    let parsed: CuratedSourceManifestFile =
        toml::from_str(curated_source_manifest_text(Some(report_id))).unwrap_or_else(|error| {
            panic!("failed to parse curated source manifest for report {report_id:?}: {error}")
        });
    for source in parsed.sources {
        for file in source.required_files {
            if curated_required_file_matches_case(&file, family, rom) {
                return (Some(source.id), Some(file.path));
            }
        }
    }

    panic!(
        "missing report-local source path for curated case {family}/{} in report {report_id:?}",
        rom.display()
    );
}

fn curated_required_file_matches_case(
    file: &CuratedRequiredFile,
    family: &str,
    rom: &Path,
) -> bool {
    if !matches!(
        file.path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("gb" | "gbc")
    ) {
        return false;
    }

    let Some((file_family, file_rom)) = curated_required_rom_path(file.clone()) else {
        return false;
    };
    file_family == family && file_rom == rom
}

fn parse_manifest_memory(
    memory: Vec<CuratedMemoryByteExpectationFile>,
) -> Vec<MemoryByteExpectation> {
    memory
        .into_iter()
        .map(|expectation| {
            if let Some(fail_value) = expectation.fail_value {
                MemoryByteExpectation::with_fail_value(
                    expectation.address,
                    expectation.value,
                    fail_value,
                )
            } else {
                MemoryByteExpectation::new(expectation.address, expectation.value)
            }
        })
        .collect()
}

fn normalize_manifest_case_comment(
    source_path: &Path,
    case_id: &str,
    disabled: bool,
    comment: Option<String>,
) -> Option<String> {
    let comment = comment
        .map(|comment| comment.trim().to_string())
        .filter(|comment| !comment.is_empty());

    if disabled && comment.is_none() {
        panic!(
            "disabled curated case {case_id} in {} must include a non-empty comment",
            source_path.display()
        );
    }

    comment
}

fn parse_manifest_timeout(
    source_path: &Path,
    timeout_frames: Option<u32>,
    timeout_tcycles: Option<u64>,
) -> Timeout {
    match (timeout_frames, timeout_tcycles) {
        (Some(frames), None) => Timeout::Frames(frames),
        (None, Some(t_cycles)) => Timeout::TCycles(t_cycles),
        (Some(_), Some(_)) => {
            panic!(
                "curated case in {} cannot specify both timeout_frames and timeout_tcycles",
                source_path.display()
            )
        }
        (None, None) => {
            panic!(
                "curated case in {} must specify timeout_frames or timeout_tcycles",
                source_path.display()
            )
        }
    }
}

#[cfg(test)]
fn parse_manifest_console_model(source_path: &str, case_id: &str, console: &str) -> ConsoleModel {
    parse_manifest_console_profile(source_path, case_id, console).0
}

#[cfg(test)]
fn parse_manifest_host_platform(source_path: &str, case_id: &str, console: &str) -> HostPlatform {
    parse_manifest_console_profile(source_path, case_id, console).1
}

fn parse_manifest_console_profile(
    source_path: &str,
    case_id: &str,
    console: &str,
) -> (ConsoleModel, HostPlatform) {
    match console {
        "game-boy" | "dmg0" | "dmg" => (ConsoleModel::GameBoy, HostPlatform::Handheld),
        "pocket" | "mgb" => (ConsoleModel::GameBoyPocket, HostPlatform::Handheld),
        "light" => (ConsoleModel::GameBoyLight, HostPlatform::Handheld),
        "color" | "cgb" => (ConsoleModel::GameBoyColor, HostPlatform::Handheld),
        "sgb" => (ConsoleModel::GameBoy, HostPlatform::Sgb),
        "sgb2" => (ConsoleModel::GameBoy, HostPlatform::Sgb2),
        other => panic!(
            "unsupported console model {other:?} for curated case {case_id} in {source_path}"
        ),
    }
}

fn parse_manifest_revision(source_path: &str, case_id: &str, revision: &str) -> HardwareRevision {
    match revision {
        "dmg-cpu-c" => HardwareRevision::DmgCpuC,
        "cpu-mgb" => HardwareRevision::CpuMgb,
        "cpu-cgb-c" => HardwareRevision::CpuCgbC,
        "cpu-cgb-d" => HardwareRevision::CpuCgbD,
        "cpu-cgb-e" => HardwareRevision::CpuCgbE,
        other => panic!(
            "unsupported hardware revision {other:?} for curated case {case_id} in {source_path}"
        ),
    }
}

fn parse_manifest_startup_mode(source_path: &str, case_id: &str, startup: &str) -> StartupMode {
    match startup {
        "skip-boot" => StartupMode::SkipBoot,
        "custom-boot" => StartupMode::CustomBoot,
        "real-boot" => StartupMode::RealBoot,
        other => {
            panic!("unsupported startup mode {other:?} for curated case {case_id} in {source_path}")
        }
    }
}

fn parse_manifest_stimulus(
    source_path: &Path,
    case_id: &str,
    stimulus: CuratedRomStimulusFile,
) -> ExternalStimulus {
    let button = parse_manifest_joypad_button(source_path, case_id, &stimulus.button);
    ExternalStimulus::at_t_cycle(
        stimulus.tcycle,
        ExternalStimulusAction::JoypadSetButton {
            button,
            pressed: stimulus.pressed,
        },
    )
}

fn parse_manifest_joypad_button(source_path: &Path, case_id: &str, button: &str) -> JoypadButton {
    match button {
        "right" => JoypadButton::Right,
        "left" => JoypadButton::Left,
        "up" => JoypadButton::Up,
        "down" => JoypadButton::Down,
        "a" => JoypadButton::A,
        "b" => JoypadButton::B,
        "select" => JoypadButton::Select,
        "start" => JoypadButton::Start,
        other => panic!(
            "unsupported joypad button {other:?} for curated case {case_id} in {}",
            source_path.display()
        ),
    }
}

fn curated_case_test_store_path(family: &str, rom: &Path, report_id: Option<&str>) -> PathBuf {
    let store_relative_path = curated_case_store_relative_path_for_report(family, rom, report_id);
    match report_id {
        Some(report_id) => Path::new(TEST_ROM_STORE_DIR)
            .join(report_id)
            .join(store_relative_path),
        None => Path::new(TEST_ROM_STORE_DIR).join(store_relative_path),
    }
}

fn required_manifest_fixture_path(
    fixture: Option<ManifestFixtureField>,
    case_id: &str,
    oracle: &str,
) -> PathBuf {
    fixture
        .unwrap_or_else(|| panic!("missing fixture path for case {case_id}"))
        .into_single_path(case_id, oracle)
        .unwrap_or_else(|error| panic!("{error}"))
}

fn required_manifest_fixture_paths(
    fixture: Option<ManifestFixtureField>,
    case_id: &str,
    oracle: &str,
) -> Vec<PathBuf> {
    fixture
        .unwrap_or_else(|| panic!("missing fixture paths for case {case_id}"))
        .into_non_empty_paths(case_id, oracle)
        .unwrap_or_else(|error| panic!("{error}"))
}

fn framebuffer_fixture_pass_condition(
    fixture: Option<ManifestFixtureField>,
    case_id: &str,
    oracle: &str,
) -> PassCondition {
    let fixture_paths = required_manifest_fixture_paths(fixture, case_id, oracle);
    match fixture_paths.as_slice() {
        [fixture_path] => PassCondition::FramebufferFixture(fixture_path.clone()),
        _ => PassCondition::FramebufferFixtureSet(fixture_paths),
    }
}

fn manifest_case_to_rom_test_case(
    case: CuratedTestRomCase,
    report_id: Option<&str>,
) -> RomTestCase {
    let CuratedTestRomCase {
        family,
        id,
        rom,
        source_id: _,
        source_path: _,
        report_console_suffix: _,
        report_label: _,
        timeout,
        oracle,
        expected,
        fixture,
        check_interval_tcycles,
        check_at_tcycles,
        memory,
        stimuli,
        console_model,
        host_platform,
        revision,
        startup_mode,
        execution_mode,
        stop_condition,
        disabled: _,
        comment: _,
    } = case;

    let pass_condition = match oracle.as_str() {
        "serial-contains" => PassCondition::SerialContains(
            expected.unwrap_or_else(|| panic!("missing expected string for case {id}")),
        ),
        "serial-hex-exact" => PassCondition::SerialHexExact(
            expected.unwrap_or_else(|| panic!("missing expected string for case {id}")),
        ),
        "blargg-console-contains" => PassCondition::BlarggConsoleTextContains(
            expected.unwrap_or_else(|| panic!("missing expected string for case {id}")),
        ),
        "blargg-memory-text-contains" => PassCondition::MemoryTextOutputContains {
            spec: blargg_memory_text_output_spec(),
            expected_substring: expected
                .unwrap_or_else(|| panic!("missing expected string for case {id}")),
        },
        "mooneye-result" => PassCondition::MooneyeResult,
        "memory-byte-equals" => PassCondition::MemoryBytesEqual(memory),
        "info-serial" => PassCondition::Informational(InformationalCaptureKind::Serial),
        "info-serial-hex" => PassCondition::Informational(InformationalCaptureKind::SerialHex),
        "info-snapshot" => PassCondition::Informational(InformationalCaptureKind::Snapshot),
        "info-framebuffer" => PassCondition::Informational(InformationalCaptureKind::Framebuffer),
        "info-trace" => PassCondition::Informational(InformationalCaptureKind::Trace),
        "framebuffer-fixture" => framebuffer_fixture_pass_condition(fixture, &id, &oracle),
        "framebuffer-fixture-until-match" => PassCondition::FramebufferFixtureUntilMatch {
            fixture_path: required_manifest_fixture_path(fixture, &id, &oracle),
            check_interval_tcycles: check_interval_tcycles.unwrap_or(100_000),
            check_at_tcycles,
        },
        "framebuffer-grayscale-fixture" => PassCondition::FramebufferGrayscaleFixture(
            required_manifest_fixture_path(fixture, &id, &oracle),
        ),
        "framebuffer-rgb555-fixture" => PassCondition::FramebufferRgb555Fixture(
            required_manifest_fixture_path(fixture, &id, &oracle),
        ),
        "framebuffer-rgb555-fixture-until-match" => {
            PassCondition::FramebufferRgb555FixtureUntilMatch {
                fixture_path: required_manifest_fixture_path(fixture, &id, &oracle),
                check_interval_tcycles: check_interval_tcycles.unwrap_or(100_000),
                check_at_tcycles,
            }
        }
        "framebuffer-rgb555-grayscale-fixture" => PassCondition::FramebufferRgb555GrayscaleFixture(
            required_manifest_fixture_path(fixture, &id, &oracle),
        ),
        "framebuffer-rgb555-grayscale-tolerance-fixture" => {
            PassCondition::FramebufferRgb555GrayscaleToleranceFixture(
                required_manifest_fixture_path(fixture, &id, &oracle),
            )
        }
        other => panic!("unsupported oracle {other:?} for case {id}"),
    };

    let capture_plan = capture_plan_for_pass_condition(&pass_condition);
    let failure_artifacts = failure_artifacts_for_pass_condition(&pass_condition);
    let mut rom_case = RomTestCase::new(
        id,
        curated_case_test_store_path(&family, &rom, report_id),
        timeout,
        pass_condition,
    )
    .with_console_model(console_model)
    .with_host_platform(host_platform)
    .with_revision(revision)
    .with_startup_mode(startup_mode)
    .with_capture_plan(capture_plan)
    .with_failure_artifacts(failure_artifacts);

    if let Some(execution_mode) = execution_mode.as_deref() {
        let case_id = rom_case.id.clone();
        rom_case = rom_case.with_execution_mode(parse_manifest_execution_mode(
            &family,
            &case_id,
            execution_mode,
        ));
    }

    if let Some(stop_condition) = stop_condition.as_deref() {
        let case_id = rom_case.id.clone();
        rom_case = rom_case.with_stop_condition(parse_manifest_stop_condition(
            &family,
            &case_id,
            stop_condition,
        ));
    }

    for stimulus in stimuli {
        rom_case = rom_case.with_external_stimulus(stimulus);
    }

    rom_case
}

fn parse_manifest_execution_mode(
    family: &str,
    case_id: &str,
    execution_mode: &str,
) -> ExecutionMode {
    match execution_mode {
        "strict" => ExecutionMode::Strict,
        "permissive" => ExecutionMode::Permissive,
        "experimental" => ExecutionMode::Experimental,
        other => panic!(
            "unsupported execution mode {other:?} for curated case {case_id} in family {family}"
        ),
    }
}

fn parse_manifest_stop_condition(
    family: &str,
    case_id: &str,
    stop_condition: &str,
) -> ExecutionStopCondition {
    match stop_condition {
        "ld-b-b" => ExecutionStopCondition::CurrentOpcodeEquals { opcode: 0x40 },
        other => panic!(
            "unsupported stop condition {other:?} for curated case {case_id} in family {family}"
        ),
    }
}

fn capture_plan_for_pass_condition(pass_condition: &PassCondition) -> CapturePlan {
    match pass_condition {
        PassCondition::SerialContains(_) | PassCondition::SerialExact(_) => CapturePlan::new()
            .with_capture(CaptureKind::Serial)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::SerialHexExact(_) => CapturePlan::new()
            .with_capture(CaptureKind::SerialHex)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::MemoryBytesEqual(_) => CapturePlan::new()
            .with_capture(CaptureKind::MemoryBytes)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::MemoryTextOutputContains { .. } => CapturePlan::new()
            .with_capture(CaptureKind::MemoryTextOutput)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::BlarggConsoleTextContains(_) => CapturePlan::new()
            .with_capture(CaptureKind::BlarggConsoleText)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::MooneyeResult => CapturePlan::new()
            .with_capture(CaptureKind::Snapshot)
            .with_capture(CaptureKind::Serial),
        PassCondition::Informational(capture) => CapturePlan::new()
            .with_capture(capture.capture_kind())
            .with_capture(CaptureKind::Snapshot),
        PassCondition::FramebufferFixture(_)
        | PassCondition::FramebufferFixtureUntilMatch { .. }
        | PassCondition::FramebufferGrayscaleFixture(_)
        | PassCondition::FramebufferRgb555Fixture(_)
        | PassCondition::FramebufferRgb555FixtureUntilMatch { .. }
        | PassCondition::FramebufferRgb555GrayscaleFixture(_)
        | PassCondition::FramebufferRgb555GrayscaleToleranceFixture(_)
        | PassCondition::FramebufferFixtureSet(_) => CapturePlan::new()
            .with_capture(CaptureKind::Framebuffer)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::TraceFixture(_) => CapturePlan::debugging_minimum_for(pass_condition),
    }
}

fn failure_artifacts_for_pass_condition(pass_condition: &PassCondition) -> FailureArtifactPolicy {
    match pass_condition {
        PassCondition::SerialContains(_) | PassCondition::SerialExact(_) => {
            FailureArtifactPolicy::new()
                .with_artifact(CaptureKind::Serial)
                .with_artifact(CaptureKind::Snapshot)
        }
        PassCondition::SerialHexExact(_) => FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::SerialHex)
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::MemoryBytesEqual(_) => FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::MemoryBytes)
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::MemoryTextOutputContains { .. } => FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::MemoryTextOutput)
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::BlarggConsoleTextContains(_) => FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::BlarggConsoleText)
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::MooneyeResult => FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Snapshot)
            .with_artifact(CaptureKind::Serial),
        PassCondition::Informational(capture) => FailureArtifactPolicy::new()
            .with_artifact(capture.capture_kind())
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::FramebufferFixture(_)
        | PassCondition::FramebufferFixtureUntilMatch { .. }
        | PassCondition::FramebufferGrayscaleFixture(_)
        | PassCondition::FramebufferRgb555Fixture(_)
        | PassCondition::FramebufferRgb555FixtureUntilMatch { .. }
        | PassCondition::FramebufferRgb555GrayscaleFixture(_)
        | PassCondition::FramebufferRgb555GrayscaleToleranceFixture(_)
        | PassCondition::FramebufferFixtureSet(_) => FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Framebuffer)
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::TraceFixture(_) => {
            FailureArtifactPolicy::debugging_minimum_for(pass_condition)
        }
    }
}

fn blargg_memory_text_output_spec() -> MemoryTextOutputSpec {
    MemoryTextOutputSpec::new(
        0xA000,
        0x80,
        0x00,
        0xA001,
        [0xDE, 0xB0, 0x61],
        0xA004,
        4_096,
    )
}

#[cfg(test)]
fn render_markdown_report(suites: &[PersistedSuiteStatus]) -> String {
    render_markdown_report_for_kind(suites, CuratedTestReportKind::Standard)
}

fn render_markdown_report_for_kind(
    suites: &[PersistedSuiteStatus],
    report_kind: CuratedTestReportKind,
) -> String {
    render_markdown_report_with_family_order(suites, report_family_order_for_kind(report_kind))
}

fn render_markdown_report_with_family_order(
    suites: &[PersistedSuiteStatus],
    family_order: &[&str],
) -> String {
    let mut ordered_suites = suites
        .iter()
        .cloned()
        .map(normalize_persisted_suite_status)
        .collect::<Vec<_>>();
    ordered_suites.sort_by(|left, right| compare_report_suites(left, right, family_order));
    let (non_failing_cases, total_cases) = report_summary_counts(&ordered_suites);
    let mut rows = ordered_suites
        .iter()
        .flat_map(|suite| {
            suite
                .cases
                .iter()
                .map(move |case| (suite.suite_name.as_str(), suite.family.as_str(), case))
        })
        .collect::<Vec<_>>();
    rows.sort_by(
        |(left_suite_name, left_default_family, left),
         (right_suite_name, right_default_family, right)| {
            let left_family = left.family.as_deref().unwrap_or(left_default_family);
            let right_family = right.family.as_deref().unwrap_or(right_default_family);
            let left_rank = report_family_rank(left_family, family_order);
            let right_rank = report_family_rank(right_family, family_order);
            let left_order = manifest_case_order(left_suite_name, left_family, &left.rom);
            let right_order = manifest_case_order(right_suite_name, right_family, &right.rom);

            (left_rank.is_none(), left_rank.unwrap_or(usize::MAX))
                .cmp(&(right_rank.is_none(), right_rank.unwrap_or(usize::MAX)))
                .then_with(|| left_family.cmp(right_family))
                .then_with(|| {
                    (
                        left_order.is_none(),
                        left_order.unwrap_or(ReportCaseOrder::fallback()),
                    )
                        .cmp(&(
                            right_order.is_none(),
                            right_order.unwrap_or(ReportCaseOrder::fallback()),
                        ))
                })
                .then_with(|| left.rom.cmp(&right.rom))
        },
    );

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Test Report ({non_failing_cases}/{total_cases})"
    );
    let _ = writeln!(&mut report);
    let _ = writeln!(&mut report, "| family | rom | status |");
    let _ = writeln!(&mut report, "| --- | --- | --- |");

    for (_, default_family, case) in rows {
        let family = case.family.as_deref().unwrap_or(default_family);
        let _ = writeln!(
            &mut report,
            "| {} | {} | {} |",
            family,
            case.rom,
            report_status_display(&case.status)
        );
    }
    report
}

fn report_summary_counts(suites: &[PersistedSuiteStatus]) -> (usize, usize) {
    let mut non_failing_cases = 0;
    let mut total_cases = 0;

    for suite in suites {
        for case in &suite.cases {
            total_cases += 1;
            if matches!(case.status.as_str(), "PASS" | "INFO") {
                non_failing_cases += 1;
            }
        }
    }

    (non_failing_cases, total_cases)
}

fn report_family_rank(family: &str, family_order: &[&str]) -> Option<usize> {
    family_order
        .iter()
        .position(|known_family| *known_family == family)
}

fn compare_report_suites(
    left: &PersistedSuiteStatus,
    right: &PersistedSuiteStatus,
    family_order: &[&str],
) -> std::cmp::Ordering {
    let left_rank = report_family_rank(&left.family, family_order);
    let right_rank = report_family_rank(&right.family, family_order);

    (left_rank.is_none(), left_rank.unwrap_or(usize::MAX))
        .cmp(&(right_rank.is_none(), right_rank.unwrap_or(usize::MAX)))
        .then_with(|| left.family.cmp(&right.family))
        .then_with(|| left.suite_name.cmp(&right.suite_name))
}

fn report_status_display(status: &str) -> &'static str {
    match status {
        "PASS" => REPORT_STATUS_PASS_EMOJI,
        "FAIL" => REPORT_STATUS_FAIL_EMOJI,
        "INFO" => REPORT_STATUS_INFO_EMOJI,
        other => panic!("unsupported curated test ROM report status {other:?}"),
    }
}

fn report_case_metadata(
    suite_name: &str,
    default_family: &str,
    case: &crate::RomCaseReport,
) -> ReportCaseMetadata {
    let (path_family, rom_without_family) = split_curated_rom_path(default_family, &case.rom_path);
    let candidates = curated_manifest_cases_for_family_rom(&path_family, &rom_without_family);

    let selected = candidates
        .iter()
        .find(|(manifest, manifest_case)| {
            manifest.suite_name == suite_name && manifest_case.id == case.case_id
        })
        .or_else(|| {
            candidates
                .iter()
                .find(|(manifest, _)| manifest.suite_name == suite_name)
        })
        .or_else(|| {
            candidates
                .iter()
                .find(|(_, manifest_case)| manifest_case.id == case.case_id)
        })
        .or_else(|| {
            candidates
                .iter()
                .find(|(manifest, _)| manifest.suite_family.as_deref() == Some(default_family))
        })
        .or_else(|| candidates.first());

    if let Some((_, manifest_case)) = selected {
        ReportCaseMetadata {
            family: manifest_case.family.clone(),
            rom: manifest_case_report_rom_display(manifest_case),
        }
    } else {
        ReportCaseMetadata {
            family: path_family,
            rom: report_rom_display(default_family, &case.rom_path),
        }
    }
}

fn split_curated_rom_path(default_family: &str, rom_path: &Path) -> (String, PathBuf) {
    let rom_path = rom_path_without_store_prefix(rom_path);
    for family in curated_test_rom_families() {
        let store_prefix = curated_family_store_prefix(&family);
        if let Ok(rom_without_family) = rom_path.strip_prefix(&store_prefix) {
            return (family, rom_without_family.to_path_buf());
        }
    }

    let mut components = rom_path.components();
    let Some(first) = components.next() else {
        return (default_family.to_string(), PathBuf::new());
    };
    let Some(first) = first.as_os_str().to_str() else {
        return (default_family.to_string(), rom_path.to_path_buf());
    };

    if curated_test_rom_families()
        .iter()
        .any(|family| family == first)
    {
        (first.to_string(), components.collect())
    } else {
        (default_family.to_string(), rom_path.to_path_buf())
    }
}

fn curated_manifest_cases_for_family_rom(
    family: &str,
    rom: &Path,
) -> Vec<(CuratedTestRomManifest, CuratedTestRomCase)> {
    let mut candidates = Vec::new();
    for manifest in curated_test_rom_manifests() {
        for case in &manifest.cases {
            if case.family == family && case.rom == rom {
                candidates.push((manifest.clone(), case.clone()));
            }
        }
    }
    candidates
}

fn manifest_case_report_rom_display(case: &CuratedTestRomCase) -> String {
    if let Some(report_label) = &case.report_label {
        return report_label.clone();
    }

    let rom = report_rom_display(
        &case.family,
        &curated_case_store_relative_path(&case.family, &case.rom),
    );
    if case.report_console_suffix {
        format!(
            "{rom} ({})",
            console_report_suffix(case.console_model, case.host_platform)
        )
    } else {
        rom
    }
}

impl ReportCaseOrder {
    fn fallback() -> Self {
        Self {
            source_order_missing: true,
            source_or_manifest_order: usize::MAX,
            console_order: usize::MAX,
            manifest_order: usize::MAX,
        }
    }
}

fn curated_source_rom_order(family: &str, rom: &Path) -> Option<usize> {
    curated_source_rom_order_catalog()
        .get(&(family.to_string(), rom.to_path_buf()))
        .copied()
}

fn curated_source_rom_order_catalog() -> &'static BTreeMap<(String, PathBuf), usize> {
    CURATED_SOURCE_ROM_ORDER_CACHE.get_or_init(|| {
        curated_source_rom_path_catalog()
            .iter()
            .enumerate()
            .map(|(order, (family, rom))| ((family.clone(), rom.clone()), order))
            .collect()
    })
}

fn curated_source_rom_path_catalog() -> &'static [(String, PathBuf)] {
    CURATED_SOURCE_ROM_PATH_CACHE
        .get_or_init(parse_curated_source_rom_paths)
        .as_slice()
}

fn parse_curated_source_rom_paths() -> Vec<(String, PathBuf)> {
    [
        (
            "legacy curated source manifest",
            include_str!("../data/sources.toml"),
        ),
        (
            "DocBoy curated source manifest",
            include_str!("../data/docboy/sources.toml"),
        ),
        (
            "gbmicrotest curated source manifest",
            include_str!("../data/gbmicrotest/sources.toml"),
        ),
        (
            "GB Emulator Shootout curated source manifest",
            include_str!("../data/gb-emulator-shootout/sources.toml"),
        ),
    ]
    .into_iter()
    .flat_map(|(label, source_manifest)| {
        parse_curated_source_rom_paths_from_text(label, source_manifest)
    })
    .collect()
}

fn parse_curated_source_rom_paths_from_text(
    label: &str,
    source_manifest: &str,
) -> Vec<(String, PathBuf)> {
    let parsed: CuratedSourceManifestFile = toml::from_str(source_manifest)
        .unwrap_or_else(|error| panic!("failed to parse {label}: {error}"));

    parsed
        .sources
        .into_iter()
        .flat_map(|source| source.required_files)
        .filter_map(curated_required_rom_path)
        .collect()
}

fn curated_required_rom_path(file: CuratedRequiredFile) -> Option<(String, PathBuf)> {
    let path = file.path;
    if !matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("gb" | "gbc")
    ) {
        return None;
    }

    if let (Some(family), Some(rom)) = (file.family, file.rom) {
        return Some((family, rom));
    }

    let stripped = path.strip_prefix(GBEMU_SHOOTOUT_TESTROMS_DIR).ok()?;
    let mut components = stripped.components();
    let family = components.next()?.as_os_str().to_str()?.to_string();
    let rom = components.collect::<PathBuf>();
    Some((family, rom))
}

fn console_report_suffix(console_model: ConsoleModel, host_platform: HostPlatform) -> &'static str {
    match host_platform {
        HostPlatform::Sgb => "SGB",
        HostPlatform::Sgb2 => "SGB2",
        HostPlatform::Handheld => match console_model {
            ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => {
                "DMG"
            }
            ConsoleModel::GameBoyColor => "GBC",
        },
    }
}

fn console_report_order(console_model: ConsoleModel, host_platform: HostPlatform) -> usize {
    match host_platform {
        HostPlatform::Handheld => match console_model {
            ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => 0,
            ConsoleModel::GameBoyColor => 1,
        },
        HostPlatform::Sgb => 2,
        HostPlatform::Sgb2 => 3,
    }
}

fn report_rom_display(family: &str, rom_path: &Path) -> String {
    let rom_path = rom_path_without_store_prefix(rom_path);
    let store_prefix = curated_family_store_prefix(family);
    if let Ok(stripped) = rom_path.strip_prefix(&store_prefix) {
        return stripped
            .to_string_lossy()
            .trim_start_matches('/')
            .to_string();
    }

    rom_path
        .strip_prefix(family)
        .unwrap_or(rom_path)
        .to_string_lossy()
        .trim_start_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::InformationalCaptureKind;

    use super::{
        CURATED_TEST_ROM_REPORT_FAMILY_ORDER, CuratedSourceManifestFile, CuratedTestReportKind,
        CuratedTestRomCase, CuratedTestRomCaseDefaultsFile, CuratedTestRomCaseFile,
        CuratedTestRomManifestFile, DOCBOY_REPORT_ID, GB_EMULATOR_SHOOTOUT_REPORT_ID,
        GBEMU_SHOOTOUT_SOURCE_ID, GBMICROTEST_REPORT_ID, PersistedCaseStatus, PersistedSuiteStatus,
        REPORT_STATUS_FAIL_EMOJI, REPORT_STATUS_INFO_EMOJI, REPORT_STATUS_PASS_EMOJI,
        TEST_ROM_DOCBOY_REPORT_DIR, TEST_ROM_DOCBOY_REPORT_FILE_NAME,
        TEST_ROM_EXTRA_REPORT_FILE_NAME, TEST_ROM_GBMICROTEST_REPORT_DIR,
        TEST_ROM_GBMICROTEST_REPORT_FILE_NAME, TEST_ROM_REPORT_FILE_NAME, TEST_ROM_ROOT_ENV_VAR,
        TEST_ROM_STATUS_DIR_NAME, TEST_ROM_STORE_DIR, ax6_dmg_extra_suite, ax6_suite,
        blargg_cgb_sound_suite, blargg_curated_suites, blargg_memory_text_output_spec,
        capture_plan_for_pass_condition, cgb_boot_hwio_suite, copy_curated_rom, cpp_suite,
        curated_source_manifest_text, curated_test_rom_families,
        curated_test_rom_families_for_report, curated_test_rom_family_suites,
        curated_test_rom_manifest_texts, curated_test_rom_manifests, discover_test_rom_store_root,
        docboy_cgb_dmg_ext_suite, docboy_cgb_dmg_suite, docboy_cgb_suite, docboy_dmg_suite,
        failure_artifacts_for_pass_condition, gbmicrotest_suite, little_things_gb_cgb_extra_suite,
        little_things_gb_dmg_extra_suite, load_persisted_suite_status, magen_cgb_extra_suite,
        manifest_case_report_rom_display, manifest_case_to_rom_test_case,
        materialize_curated_test_rom_families, materialize_curated_test_rom_store,
        mealybug_tearoom_cgb_extra_suite, mooneye_cgb_extra_suite,
        mooneye_sgb_boot_regs_extra_suite, parse_manifest, parse_manifest_case,
        parse_manifest_console_model, parse_manifest_host_platform, render_markdown_report,
        report_family_order_for_kind, report_family_rank, report_rom_display,
        report_status_display, required_file_family, rom_path_without_store_prefix,
        samesuite_apu_suite, samesuite_cgb_extra_suite, samesuite_dmg_extra_suite, samesuite_suite,
        sort_persisted_case_statuses, suite_report_id, suite_uses_docboy_test_report,
        suite_uses_extra_test_report, suite_uses_gbmicrotest_test_report, test_rom_store_root,
        test_rom_store_root_for_report, update_curated_test_report,
    };
    use crate::manifest_fixture::ManifestFixtureField;
    use crate::{
        CaptureKind, CapturedArtifacts, MemoryByteExpectation, PassCondition, RomCaseFailure,
        RomCaseOutcome, RomCaseReport, RomSuiteReport, Timeout,
    };
    use gb_core::{ConsoleModel, HardwareRevision, HostPlatform, StartupMode};
    use std::collections::BTreeSet;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-curated-test-roms-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn gbemu_report_root(workspace_root: &Path) -> PathBuf {
        test_rom_store_root_for_report(workspace_root, GB_EMULATOR_SHOOTOUT_REPORT_ID)
    }

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore the touched variables before dropping the guard.
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore the touched variables before dropping the guard.
        unsafe {
            env::remove_var(key);
        }
    }

    fn report_case(case_id: &str, rom_path: &str, outcome: RomCaseOutcome) -> RomCaseReport {
        RomCaseReport {
            case_id: case_id.to_string(),
            rom_path: PathBuf::from(rom_path),
            outcome,
            executed_t_cycles: 0,
            completed_frames: 0,
            diagnostics: Vec::new(),
            artifacts: CapturedArtifacts::default(),
            retained_failure_artifacts: Vec::new(),
        }
    }

    fn collect_fixture_files(root: &Path, current: &Path, fixtures: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(current).expect("fixture directory should be readable") {
            let entry = entry.expect("fixture directory entry should be readable");
            let path = entry.path();
            if path.is_dir() {
                collect_fixture_files(root, &path, fixtures);
            } else {
                fixtures.push(
                    path.strip_prefix(root)
                        .expect("fixture should be below root")
                        .to_path_buf(),
                );
            }
        }
    }

    fn write_fake_gbemu_shootout_tree(root: &Path) {
        for manifest in curated_test_rom_manifests() {
            for case in manifest
                .cases
                .iter()
                .filter(|case| case.source_id == GBEMU_SHOOTOUT_SOURCE_ID)
            {
                let source_path = root.join(&case.source_path);
                let source_parent = source_path
                    .parent()
                    .expect("curated ROM path should always have a parent");
                fs::create_dir_all(source_parent)
                    .expect("fake shootout ROM parent should be creatable");
                fs::write(
                    &source_path,
                    format!("{}:{}", case.family, case.rom.display()),
                )
                .expect("fake shootout ROM should be writable");
            }
        }
    }

    #[test]
    fn curated_materialization_error_paths_report_target_context() {
        let workspace_file = unique_temp_dir("materialize-workspace-file");
        fs::write(&workspace_file, "not-a-directory").expect("workspace file should be writable");
        let gbemu_shootout_root = unique_temp_dir("materialize-workspace-file-source");
        let error = materialize_curated_test_rom_store(&workspace_file, &gbemu_shootout_root)
            .expect_err("workspace file should reject store creation");
        assert!(error.contains("failed to create curated test ROM store"));
        fs::remove_file(&workspace_file).expect("workspace file should be removable");

        let workspace_root = unique_temp_dir("materialize-replace-file");
        let gbemu_shootout_root = workspace_root.join("gbemu-shootout");
        write_fake_gbemu_shootout_tree(&gbemu_shootout_root);
        let ax6_root = test_rom_store_root(&workspace_root).join("ax6");
        fs::create_dir_all(ax6_root.parent().expect("AX6 root should have a parent"))
            .expect("store root should be creatable");
        fs::write(&ax6_root, "not-a-directory").expect("AX6 file should be writable");
        let error = materialize_curated_test_rom_families(
            &workspace_root,
            &gbemu_shootout_root,
            &["ax6".to_string()],
        )
        .expect_err("family file should reject replacement");
        assert!(error.contains("failed to replace curated family directory"));

        let blocked_parent = workspace_root.join("blocked-parent");
        fs::write(&blocked_parent, "not-a-directory").expect("blocked parent should be writable");
        let source_rom = gbemu_shootout_root.join("testroms/ax6/rtc3test-1.gb");
        assert!(source_rom.exists());
        let error = copy_curated_rom(
            &gbemu_shootout_root,
            "ax6",
            Path::new("rtc3test-1.gb"),
            &blocked_parent.join("rtc3test-1.gb"),
        )
        .expect_err("file parent should reject copied ROM target");
        assert!(error.contains("failed to create curated ROM parent"));

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn curated_blargg_manifest_tracks_the_full_individual_shootout_list() {
        let split_suites = blargg_curated_suites();
        let cases = split_suites
            .iter()
            .flat_map(|suite| suite.cases.iter())
            .collect::<Vec<_>>();

        assert_eq!(
            split_suites
                .iter()
                .map(|suite| suite.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "blargg-cpu-instrs",
                "blargg-dmg-sound",
                "blargg-timing-memory-oam"
            ]
        );
        assert!(
            split_suites
                .iter()
                .all(|suite| suite.family.as_deref() == Some("blargg"))
        );
        assert_eq!(cases.len(), 39);
        assert!(cases.iter().any(|case| case.id == "blargg-instr-timing"));
        assert!(cases.iter().any(|case| case.id == "blargg-interrupt-time"));
        assert!(
            cases
                .iter()
                .any(|case| case.id == "blargg-dmg-sound-12-wave-write-while-on")
        );
        assert!(cases.iter().all(|case| {
            rom_path_without_store_prefix(&case.rom_path).starts_with(Path::new("blargg"))
        }));
    }

    #[test]
    fn curated_manifest_cases_resolve_console_explicitly() {
        for (source_path, source_text) in curated_test_rom_manifest_texts() {
            let manifest: CuratedTestRomManifestFile = toml::from_str(source_text)
                .unwrap_or_else(|error| panic!("failed to parse {source_path}: {error}"));
            let missing_console = manifest
                .cases
                .iter()
                .filter(|case| case.console.is_none() && manifest.defaults.console.is_none())
                .map(|case| case.id.as_str())
                .collect::<Vec<_>>();

            assert!(
                missing_console.is_empty(),
                "{source_path} cases missing case-level or manifest-level console: {missing_console:?}"
            );
        }
    }

    #[test]
    fn cgb_boot_hwio_suite_is_manifest_backed_and_internal_mooneye_gate() {
        let suite = cgb_boot_hwio_suite();

        assert_eq!(suite.name, "cgb-boot-hwio");
        assert_eq!(suite.family.as_deref(), Some("cgb-boot-hwio"));
        assert_eq!(suite.cases.len(), 1);
        assert_eq!(
            rom_path_without_store_prefix(&suite.cases[0].rom_path),
            Path::new("mooneye/misc/boot_hwio-C.gb")
        );
        assert_eq!(suite.cases[0].console_model, ConsoleModel::GameBoyColor);
        assert_eq!(suite.cases[0].startup_mode, StartupMode::SkipBoot);
        assert!(matches!(
            suite.cases[0].pass_condition,
            PassCondition::MooneyeResult
        ));
    }

    #[test]
    fn mooneye_cgb_extra_suite_runs_the_ppu_acceptance_rows_on_cgb() {
        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "mooneye-cgb-extra")
            .expect("Mooneye CGB manifest should exist");
        assert_eq!(manifest.cases.len(), 12);
        assert_eq!(
            manifest.cases.iter().filter(|case| case.disabled).count(),
            2
        );
        assert!(manifest.cases.iter().any(|case| {
            case.id == "mooneye-cgb-ppu-lcdon-timing-gs"
                && case.disabled
                && case.comment.as_deref().is_some_and(|comment| {
                    comment.contains("Expected CGB red")
                        && comment.contains("STAT LYC=1")
                        && comment.contains("$6F")
                })
        }));
        assert!(manifest.cases.iter().any(|case| {
            case.id == "mooneye-cgb-ppu-vblank-stat-intr-gs"
                && case.disabled
                && case.comment.as_deref().is_some_and(|comment| {
                    comment.contains("Expected CGB red")
                        && comment.contains("D=$12")
                        && comment.contains("D=$01")
                })
        }));

        let suite = mooneye_cgb_extra_suite();

        assert_eq!(suite.name, "mooneye-cgb-extra");
        assert_eq!(suite.family.as_deref(), Some("mooneye"));
        assert_eq!(suite.cases.len(), 10);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
                && rom_path_without_store_prefix(&case.rom_path)
                    .starts_with(Path::new("mooneye/acceptance/ppu"))
                && matches!(case.pass_condition, PassCondition::MooneyeResult)
        }));
        assert!(suite.cases.iter().any(|case| {
            case.id == "mooneye-cgb-ppu-intr-2-mode0-timing"
                && rom_path_without_store_prefix(&case.rom_path)
                    == Path::new("mooneye/acceptance/ppu/intr_2_mode0_timing.gb")
                && case.timeout == Timeout::Frames(180)
        }));
        assert!(suite.cases.iter().any(|case| {
            case.id == "mooneye-cgb-ppu-intr-2-mode0-timing-sprites"
                && case.timeout == Timeout::Frames(660)
        }));
        assert!(suite.cases.iter().any(|case| {
            case.id == "mooneye-cgb-ppu-lcdon-write-timing-gs"
                && case.timeout == Timeout::Frames(240)
        }));
        assert!(suite.cases.iter().all(|case| {
            case.id != "mooneye-cgb-ppu-lcdon-timing-gs"
                && case.id != "mooneye-cgb-ppu-vblank-stat-intr-gs"
        }));
        assert!(crate::built_in_rom_suite_by_name("mooneye-cgb-extra").is_some());
        assert!(suite_uses_extra_test_report("mooneye-cgb-extra"));
        assert!(!suite_uses_docboy_test_report("mooneye-cgb-extra"));
    }

    #[test]
    fn mooneye_sgb_boot_regs_extra_suite_runs_sgb_profiles_as_extra_rows() {
        let suite = mooneye_sgb_boot_regs_extra_suite();

        assert_eq!(suite.name, "mooneye-sgb-boot-regs-extra");
        assert_eq!(suite.family.as_deref(), Some("mooneye-sgb-boot-regs-extra"));
        assert_eq!(suite.cases.len(), 2);
        assert!(crate::built_in_rom_suite_by_name("mooneye-sgb-boot-regs-extra").is_some());
        assert!(suite_uses_extra_test_report("mooneye-sgb-boot-regs-extra"));
        assert!(!suite_uses_docboy_test_report(
            "mooneye-sgb-boot-regs-extra"
        ));

        let expected = [
            (
                "mooneye-sgb-boot-regs-sgb",
                HostPlatform::Sgb,
                "acceptance/boot_regs-sgb.gb",
            ),
            (
                "mooneye-sgb-boot-regs-sgb2",
                HostPlatform::Sgb2,
                "acceptance/boot_regs-sgb2.gb",
            ),
        ];
        for (case, (id, host_platform, rom_path)) in suite.cases.iter().zip(expected) {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, ConsoleModel::GameBoy);
            assert_eq!(case.host_platform, host_platform);
            assert_eq!(case.startup_mode, StartupMode::SkipBoot);
            assert_eq!(case.timeout, Timeout::Frames(180));
            assert_eq!(
                rom_path_without_store_prefix(&case.rom_path),
                Path::new("mooneye").join(rom_path)
            );
            assert_eq!(case.pass_condition, PassCondition::MooneyeResult);
            assert!(case.capture_plan.contains(CaptureKind::Serial));
            assert!(case.capture_plan.contains(CaptureKind::Snapshot));
            assert!(case.failure_artifacts.contains(CaptureKind::Serial));
            assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
        }
    }

    #[test]
    fn ax6_suite_promotes_slice8_rows_to_blocking_oracles() {
        let suite = ax6_suite();

        assert_eq!(suite.name, "ax6");
        assert_eq!(suite.family.as_deref(), Some("ax6"));
        assert_eq!(suite.cases.len(), 3);

        let expected = [
            (
                "ax6-rtc3test-1",
                "ax6/rtc3test-1.gb",
                Timeout::Frames(1140),
                "test/gb-emulator-shootout/ax6/rtc3test-1.png",
            ),
            (
                "ax6-rtc3test-2",
                "ax6/rtc3test-2.gb",
                Timeout::Frames(900),
                "test/gb-emulator-shootout/ax6/rtc3test-2.png",
            ),
            (
                "ax6-rtc3test-3",
                "ax6/rtc3test-3.gb",
                Timeout::Frames(2400),
                "test/gb-emulator-shootout/ax6/rtc3test-3.png",
            ),
        ];

        for (case, (id, rom_path, timeout, fixture_path)) in suite.cases.iter().zip(expected) {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
            assert_eq!(
                rom_path_without_store_prefix(&case.rom_path),
                Path::new(rom_path)
            );
            assert_eq!(case.timeout, timeout);
            assert_eq!(
                case.pass_condition,
                PassCondition::FramebufferRgb555Fixture(PathBuf::from(fixture_path))
            );
        }
    }

    #[test]
    fn ax6_dmg_extra_suite_forces_dmg_model_and_report_suffixes() {
        let suite = ax6_dmg_extra_suite();

        assert_eq!(suite.name, "ax6-dmg-extra");
        assert_eq!(suite.family.as_deref(), Some("ax6"));
        assert_eq!(suite.cases.len(), 3);

        let expected = [
            (
                "ax6-dmg-rtc3test-1",
                "ax6/rtc3test-1.gb",
                Timeout::Frames(1140),
                "crates/gb-test-runner/data/ax6/fixtures/rtc3test-1.dmg.png",
                "rtc3test-1.gb (DMG)",
            ),
            (
                "ax6-dmg-rtc3test-2",
                "ax6/rtc3test-2.gb",
                Timeout::Frames(900),
                "crates/gb-test-runner/data/ax6/fixtures/rtc3test-2.dmg.png",
                "rtc3test-2.gb (DMG)",
            ),
            (
                "ax6-dmg-rtc3test-3",
                "ax6/rtc3test-3.gb",
                Timeout::Frames(2400),
                "crates/gb-test-runner/data/ax6/fixtures/rtc3test-3.dmg.png",
                "rtc3test-3.gb (DMG)",
            ),
        ];
        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "ax6-dmg-extra")
            .expect("AX6 DMG extra manifest should exist");

        for ((case, manifest_case), (id, rom_path, timeout, fixture_path, report_rom)) in
            suite.cases.iter().zip(&manifest.cases).zip(expected)
        {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, ConsoleModel::GameBoy);
            assert_eq!(
                rom_path_without_store_prefix(&case.rom_path),
                Path::new(rom_path)
            );
            assert_eq!(case.timeout, timeout);
            assert_eq!(
                case.pass_condition,
                PassCondition::FramebufferFixture(PathBuf::from(fixture_path))
            );
            assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
            assert!(case.capture_plan.contains(CaptureKind::Snapshot));
            assert!(case.failure_artifacts.contains(CaptureKind::Framebuffer));
            assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
            assert_eq!(manifest_case_report_rom_display(manifest_case), report_rom);
        }
    }

    #[test]
    fn samesuite_dmg_extra_suite_forces_dmg_model_and_report_suffixes() {
        let suite = samesuite_dmg_extra_suite();

        assert_eq!(suite.name, "samesuite-dmg-extra");
        assert_eq!(suite.family.as_deref(), Some("samesuite"));
        assert_eq!(suite.cases.len(), 3);

        let expected = [
            (
                "samesuite-dmg-div-write-trigger",
                "samesuite/apu/div_write_trigger.gb",
                "crates/gb-test-runner/data/samesuite/fixtures/dmg/apu/div_write_trigger.png",
                "apu/div_write_trigger.gb (DMG)",
            ),
            (
                "samesuite-dmg-div-write-trigger-10",
                "samesuite/apu/div_write_trigger_10.gb",
                "crates/gb-test-runner/data/samesuite/fixtures/dmg/apu/div_write_trigger_10.png",
                "apu/div_write_trigger_10.gb (DMG)",
            ),
            (
                "samesuite-dmg-ei-delay-halt",
                "samesuite/interrupt/ei_delay_halt.gb",
                "crates/gb-test-runner/data/samesuite/fixtures/dmg/interrupt/ei_delay_halt.png",
                "interrupt/ei_delay_halt.gb",
            ),
        ];
        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "samesuite-dmg-extra")
            .expect("SameSuite DMG extra manifest should exist");

        for ((case, manifest_case), (id, rom_path, fixture_path, report_rom)) in
            suite.cases.iter().zip(&manifest.cases).zip(expected)
        {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, ConsoleModel::GameBoy);
            assert_eq!(
                rom_path_without_store_prefix(&case.rom_path),
                Path::new(rom_path)
            );
            assert_eq!(case.timeout, Timeout::Frames(180));
            assert_eq!(
                case.pass_condition,
                PassCondition::FramebufferFixture(PathBuf::from(fixture_path))
            );
            assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
            assert!(case.capture_plan.contains(CaptureKind::Snapshot));
            assert!(case.failure_artifacts.contains(CaptureKind::Framebuffer));
            assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
            assert_eq!(manifest_case_report_rom_display(manifest_case), report_rom);
        }
    }

    #[test]
    fn samesuite_suite_collects_promoted_sgb_cgb_ppu_and_dma_rows() {
        let suite = samesuite_suite();

        assert_eq!(suite.name, "samesuite");
        assert_eq!(suite.family.as_deref(), Some("samesuite"));
        assert_eq!(suite.cases.len(), 7);
        assert!(crate::built_in_rom_suite_by_name("samesuite").is_some());
        assert!(!suite_uses_extra_test_report("samesuite"));
        assert!(!suite_uses_docboy_test_report("samesuite"));

        let expected = [
            (
                "samesuite-sgb-command-mlt-req",
                "samesuite/sgb/command_mlt_req.gb",
                ConsoleModel::GameBoy,
                HostPlatform::Sgb,
                Timeout::Frames(300),
                PassCondition::FramebufferFixture(PathBuf::from(
                    "crates/gb-test-runner/data/gb-emulator-shootout/fixtures/samesuite/sgb/command_mlt_req.png",
                )),
            ),
            (
                "samesuite-sgb-command-mlt-req-1-incrementing",
                "samesuite/sgb/command_mlt_req_1_incrementing.gb",
                ConsoleModel::GameBoy,
                HostPlatform::Sgb,
                Timeout::Frames(180),
                PassCondition::FramebufferFixture(PathBuf::from(
                    "crates/gb-test-runner/data/gb-emulator-shootout/fixtures/samesuite/sgb/command_mlt_req_1_incrementing.png",
                )),
            ),
            (
                "samesuite-ppu-blocking-bgpi-increase",
                "samesuite/ppu/blocking_bgpi_increase.gb",
                ConsoleModel::GameBoyColor,
                HostPlatform::Handheld,
                Timeout::Frames(180),
                PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                    "test/gb-emulator-shootout/samesuite/ppu/blocking_bgpi_increase.png",
                )),
            ),
            (
                "samesuite-dma-gbc-dma-cont",
                "samesuite/dma/gbc_dma_cont.gb",
                ConsoleModel::GameBoyColor,
                HostPlatform::Handheld,
                Timeout::Frames(180),
                PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                    "test/gb-emulator-shootout/samesuite/dma/gbc_dma_cont.png",
                )),
            ),
            (
                "samesuite-dma-gdma-addr-mask",
                "samesuite/dma/gdma_addr_mask.gb",
                ConsoleModel::GameBoyColor,
                HostPlatform::Handheld,
                Timeout::Frames(180),
                PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                    "test/gb-emulator-shootout/samesuite/dma/gdma_addr_mask.png",
                )),
            ),
            (
                "samesuite-dma-hdma-lcd-off",
                "samesuite/dma/hdma_lcd_off.gb",
                ConsoleModel::GameBoyColor,
                HostPlatform::Handheld,
                Timeout::Frames(180),
                PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                    "test/gb-emulator-shootout/samesuite/dma/hdma_lcd_off.png",
                )),
            ),
            (
                "samesuite-dma-hdma-mode0",
                "samesuite/dma/hdma_mode0.gb",
                ConsoleModel::GameBoyColor,
                HostPlatform::Handheld,
                Timeout::Frames(180),
                PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                    "test/gb-emulator-shootout/samesuite/dma/hdma_mode0.png",
                )),
            ),
        ];

        for (case, (id, rom_path, console_model, host_platform, timeout, pass_condition)) in
            suite.cases.iter().zip(expected)
        {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, console_model);
            assert_eq!(case.host_platform, host_platform);
            assert_eq!(case.startup_mode, StartupMode::SkipBoot);
            assert_eq!(case.timeout, timeout);
            assert_eq!(
                rom_path_without_store_prefix(&case.rom_path),
                Path::new(rom_path)
            );
            assert_eq!(case.pass_condition, pass_condition);
        }
    }

    #[test]
    fn cpp_suite_runs_sgb_fixture_row_on_sgb_host() {
        let suite = cpp_suite();

        assert_eq!(suite.name, "cpp");
        assert_eq!(suite.family.as_deref(), Some("cpp"));
        assert_eq!(suite.cases.len(), 4);
        assert!(crate::built_in_rom_suite_by_name("cpp").is_some());
        assert!(!suite_uses_extra_test_report("cpp"));
        assert!(!suite_uses_docboy_test_report("cpp"));

        let case = suite
            .cases
            .iter()
            .find(|case| case.id == "cpp-sgb-ext-test")
            .expect("cpp SGB case should be part of the cpp suite");
        assert_eq!(case.id, "cpp-sgb-ext-test");
        assert_eq!(case.console_model, ConsoleModel::GameBoy);
        assert_eq!(case.host_platform, HostPlatform::Sgb);
        assert_eq!(case.startup_mode, StartupMode::SkipBoot);
        assert_eq!(case.timeout, Timeout::Frames(240));
        assert_eq!(
            rom_path_without_store_prefix(&case.rom_path),
            Path::new("cpp/sgb-ext-test.gb")
        );
        assert_eq!(
            case.pass_condition,
            PassCondition::FramebufferFixture(PathBuf::from(
                "crates/gb-test-runner/data/gb-emulator-shootout/fixtures/cpp/sgb-ext-test.sgb.png"
            ))
        );
        assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
        assert!(case.capture_plan.contains(CaptureKind::Snapshot));
        assert!(case.failure_artifacts.contains(CaptureKind::Framebuffer));
        assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
    }

    #[test]
    fn cpp_sgb_reference_fixture_matches_upstream_pass_signature() {
        const TILE_0: [u8; 8] = [0x00, 0xEE, 0xAA, 0xAA, 0xAA, 0xAA, 0xEE, 0x00];
        const TILE_1: [u8; 8] = [0x00, 0xE4, 0xAC, 0xA4, 0xA4, 0xA4, 0xEE, 0x00];
        const TILE_2: [u8; 8] = [0x00, 0xEE, 0xAA, 0xA2, 0xAE, 0xA8, 0xEE, 0x00];
        const TILE_4: [u8; 8] = [0x00, 0xEA, 0xAA, 0xAE, 0xA2, 0xA2, 0xE2, 0x00];
        const PASS_VALUES: [u8; 27] = [
            0x04, 0x01, 0x04, 0x01, 0x01, 0x01, 0x04, 0x02, 0x02, 0x01, 0x01, 0x01, 0x01, 0x01,
            0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
        ];

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data/gb-emulator-shootout/fixtures/cpp/sgb-ext-test.sgb.png");
        let fixture = crate::framebuffer_oracle::decode_fixture_framebuffer_path(&fixture_path)
            .expect("cpp SGB reference fixture should decode");

        assert_eq!(fixture.width, 160);
        assert_eq!(fixture.height, 144);

        let mut expected = vec![0_u8; 160 * 144];
        for tile_y in 0..18 {
            for tile_x in 0..20 {
                let output_index = tile_y * 16 + tile_x;
                let tile = if tile_x < 16 && output_index < PASS_VALUES.len() {
                    PASS_VALUES[output_index]
                } else {
                    0x00
                };
                let rows = match tile {
                    0x00 => TILE_0,
                    0x01 => TILE_1,
                    0x02 => TILE_2,
                    0x04 => TILE_4,
                    _ => panic!("unexpected cpp SGB pass fixture tile {tile:#04X}"),
                };
                for (row, bits) in rows.iter().enumerate() {
                    for col in 0..8 {
                        if *bits & (0x80_u8 >> col) != 0 {
                            expected[(tile_y * 8 + row) * 160 + tile_x * 8 + col] = 1;
                        }
                    }
                }
            }
        }

        assert_eq!(fixture.palette_ranks, expected);
    }

    #[test]
    fn little_things_gb_dmg_extra_suite_uses_skip_boot_logo_seed() {
        let suite = little_things_gb_dmg_extra_suite();

        assert_eq!(suite.name, "little-things-gb-dmg-extra");
        assert_eq!(suite.family.as_deref(), Some("little-things-gb"));
        assert_eq!(suite.cases.len(), 2);

        let expected = [
            (
                "little-things-gb-dmg-double-halt-cancel",
                "little-things-gb/double-halt-cancel.gb",
                "crates/gb-test-runner/data/little-things-gb/fixtures/dmg/double-halt-cancel.png",
                "double-halt-cancel.gb",
                StartupMode::SkipBoot,
            ),
            (
                "little-things-gb-dmg-whichboot",
                "little-things-gb/whichboot.gb",
                "crates/gb-test-runner/data/little-things-gb/fixtures/dmg/whichboot.png",
                "whichboot.gb",
                StartupMode::SkipBoot,
            ),
        ];
        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "little-things-gb-dmg-extra")
            .expect("little-things-gb DMG extra manifest should exist");

        for ((case, manifest_case), (id, rom_path, fixture_path, report_rom, startup_mode)) in
            suite.cases.iter().zip(&manifest.cases).zip(expected)
        {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, ConsoleModel::GameBoy);
            assert_eq!(
                rom_path_without_store_prefix(&case.rom_path),
                Path::new(rom_path)
            );
            assert_eq!(case.timeout, Timeout::Frames(180));
            assert_eq!(
                case.pass_condition,
                PassCondition::FramebufferFixture(PathBuf::from(fixture_path))
            );
            assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
            assert!(case.capture_plan.contains(CaptureKind::Snapshot));
            assert!(case.failure_artifacts.contains(CaptureKind::Framebuffer));
            assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
            assert_eq!(case.startup_mode, startup_mode);
            assert!(case.startup_memory_writes.is_empty());
            assert_eq!(manifest_case_report_rom_display(manifest_case), report_rom);
        }
    }

    #[test]
    fn little_things_gb_cgb_extra_suite_runs_whichboot_on_cgb() {
        let suite = little_things_gb_cgb_extra_suite();

        assert_eq!(suite.name, "little-things-gb-cgb-extra");
        assert_eq!(suite.family.as_deref(), Some("little-things-gb"));
        assert_eq!(suite.cases.len(), 1);

        let case = &suite.cases[0];
        assert_eq!(case.id, "little-things-gb-cgb-whichboot");
        assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
        assert_eq!(
            rom_path_without_store_prefix(&case.rom_path),
            Path::new("little-things-gb/whichboot.gb")
        );
        assert_eq!(case.timeout, Timeout::Frames(180));
        assert_eq!(
            case.pass_condition,
            PassCondition::FramebufferFixture(PathBuf::from(
                "crates/gb-test-runner/data/little-things-gb/fixtures/cgb/whichboot.png"
            ))
        );
        assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
        assert!(case.capture_plan.contains(CaptureKind::Snapshot));
        assert!(case.failure_artifacts.contains(CaptureKind::Framebuffer));
        assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
        assert_eq!(case.startup_mode, StartupMode::CustomBoot);
        assert!(case.startup_memory_writes.is_empty());

        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "little-things-gb-cgb-extra")
            .expect("little-things-gb CGB extra manifest should exist");
        assert_eq!(manifest.cases.len(), 1);
        assert_eq!(manifest.cases[0].source_id, "docboy");
        assert_eq!(
            manifest.cases[0].source_path,
            Path::new("tests/roms/dmg/little-things-gb/whichboot.gb")
        );
        assert!(manifest.cases[0].report_console_suffix);
        assert_eq!(
            manifest_case_report_rom_display(&manifest.cases[0]),
            "whichboot.gb (GBC)"
        );
        assert!(suite_uses_extra_test_report("little-things-gb-cgb-extra"));
        assert!(!suite_uses_docboy_test_report("little-things-gb-cgb-extra"));
    }

    #[test]
    fn gbmicrotest_suite_marks_only_reset_facing_rows_custom_boot() {
        const CUSTOM_BOOT_PPU_TIMING_ROMS: &[&str] = &[
            "ppu/hblank_int_scx0_if_a.gb",
            "ppu/hblank_int_scx0_if_b.gb",
            "ppu/hblank_int_scx0_if_c.gb",
            "ppu/hblank_int_scx1_if_a.gb",
            "ppu/hblank_int_scx1_if_b.gb",
            "ppu/hblank_int_scx1_if_c.gb",
            "ppu/hblank_int_scx1_nops_b.gb",
            "ppu/hblank_int_scx2_if_a.gb",
            "ppu/hblank_int_scx2_if_b.gb",
            "ppu/hblank_int_scx2_if_c.gb",
            "ppu/hblank_int_scx2_nops_b.gb",
            "ppu/hblank_int_scx3_if_a.gb",
            "ppu/hblank_int_scx3_if_b.gb",
            "ppu/hblank_int_scx3_if_c.gb",
            "ppu/hblank_int_scx3_nops_b.gb",
            "ppu/hblank_int_scx4_if_a.gb",
            "ppu/hblank_int_scx4_if_b.gb",
            "ppu/hblank_int_scx4_if_c.gb",
            "ppu/hblank_int_scx4_nops_b.gb",
            "ppu/hblank_int_scx5_if_a.gb",
            "ppu/hblank_int_scx5_if_b.gb",
            "ppu/hblank_int_scx5_if_c.gb",
            "ppu/hblank_int_scx5_nops_b.gb",
            "ppu/hblank_int_scx6_if_a.gb",
            "ppu/hblank_int_scx6_if_b.gb",
            "ppu/hblank_int_scx6_if_c.gb",
            "ppu/hblank_int_scx6_nops_b.gb",
            "ppu/hblank_int_scx7_if_a.gb",
            "ppu/hblank_int_scx7_if_b.gb",
            "ppu/hblank_int_scx7_if_c.gb",
            "ppu/hblank_int_scx7_nops_b.gb",
            "ppu/line_65_ly.gb",
        ];

        let manifest_text = include_str!("../data/gbmicrotest/gbmicrotest.toml");
        assert!(
            manifest_text.matches("startup = \"custom-boot\"").count()
                == 62 + CUSTOM_BOOT_PPU_TIMING_ROMS.len(),
            "gbmicrotest should use CustomBoot only for reset-facing PPU rows"
        );
        assert!(
            !manifest_text.contains("startup_ppu_profile"),
            "gbmicrotest should rely on core CustomBoot PPU publication instead of runner profiles"
        );
        assert!(!manifest_text.contains("source_id ="));
        assert!(!manifest_text.contains("source_path ="));
        assert_eq!(
            manifest_text
                .matches("oracle = \"memory-byte-equals\"")
                .count(),
            1
        );
        assert_eq!(
            manifest_text
                .matches("memory = [{ address = 65410, value = 1 }]")
                .count(),
            1
        );

        let suite = gbmicrotest_suite();

        assert_eq!(suite.name, "gbmicrotest");
        assert_eq!(suite.family.as_deref(), Some("gbmicrotest"));
        assert_eq!(suite.report_id.as_deref(), Some(GBMICROTEST_REPORT_ID));
        assert_eq!(suite.cases.len(), 438);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoy
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
                && case.rom_path.starts_with(Path::new("test/gbmicrotest"))
                && case.capture_plan.contains(CaptureKind::MemoryBytes)
                && case.capture_plan.contains(CaptureKind::Snapshot)
                && case.failure_artifacts.contains(CaptureKind::MemoryBytes)
                && case.failure_artifacts.contains(CaptureKind::Snapshot)
                && !rom_path_without_store_prefix(&case.rom_path).starts_with("gbmicrotest")
                && case.pass_condition
                    == PassCondition::MemoryBytesEqual(vec![MemoryByteExpectation::new(
                        0xFF82, 0x01,
                    )])
        }));
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| {
                    rom_path_without_store_prefix(&case.rom_path)
                        .to_string_lossy()
                        .starts_with("boot/poweron_")
                        && case.startup_mode == StartupMode::CustomBoot
                })
                .count(),
            62
        );
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| {
                    CUSTOM_BOOT_PPU_TIMING_ROMS.iter().any(|rom_path| {
                        rom_path_without_store_prefix(&case.rom_path) == Path::new(rom_path)
                    }) && case.startup_mode == StartupMode::CustomBoot
                })
                .count(),
            CUSTOM_BOOT_PPU_TIMING_ROMS.len()
        );
        assert!(suite.cases.iter().all(|case| {
            let reset_facing_ppu_timing_row = CUSTOM_BOOT_PPU_TIMING_ROMS.iter().any(|rom_path| {
                rom_path_without_store_prefix(&case.rom_path) == Path::new(rom_path)
            });
            case.startup_mode
                == if rom_path_without_store_prefix(&case.rom_path)
                    .to_string_lossy()
                    .starts_with("boot/poweron_")
                    || reset_facing_ppu_timing_row
                {
                    StartupMode::CustomBoot
                } else {
                    StartupMode::SkipBoot
                }
        }));
        let dma_rows = [
            "dma/dma_0x1000.gb",
            "dma/dma_0x9000.gb",
            "dma/dma_0xA000.gb",
            "dma/dma_0xC000.gb",
            "dma/dma_0xE000.gb",
            "dma/dma_timing_a.gb",
        ];
        for rom_path in dma_rows {
            assert!(
                suite.cases.iter().any(
                    |case| rom_path_without_store_prefix(&case.rom_path) == Path::new(rom_path)
                ),
                "{rom_path} should be materialized from DocBoy's on-disk gbmicrotest/dma ROMs"
            );
        }
        let long_spin_if_ime0 = suite
            .cases
            .iter()
            .find(|case| case.id == "gbmicrotest-interrupts-is-if-set-during-ime0")
            .expect("long IME=0 IF visibility row should stay in the DocBoy manifest");
        assert_eq!(long_spin_if_ime0.timeout, Timeout::Frames(30));
        assert!(suite.cases.iter().all(|case| {
            case.id == long_spin_if_ime0.id || case.timeout == Timeout::Frames(15)
        }));
        assert!(suite_uses_gbmicrotest_test_report("gbmicrotest"));
        assert!(!suite_uses_extra_test_report("gbmicrotest"));
        assert!(crate::built_in_rom_suite_by_name("gbmicrotest").is_some());
        assert!(crate::built_in_rom_suite_by_name("gbmicrotest-dmg-extra").is_none());
    }

    #[test]
    fn docboy_dmg_suite_tracks_single_machine_docboy_rows() {
        let manifest_text = include_str!("../data/docboy/docboy-dmg.toml");
        assert!(
            !manifest_text.contains("startup ="),
            "docboy manifest must stay startup-neutral so Make targets choose SkipBoot or RealBoot"
        );
        assert!(
            !manifest_text.contains("startup_ppu_profile"),
            "docboy-dmg should not rely on runner-only PPU profiles"
        );

        let suite = docboy_dmg_suite();

        assert_eq!(suite.name, "docboy-dmg");
        assert_eq!(suite.family.as_deref(), Some("docboy-dmg"));
        assert_eq!(suite.cases.len(), 2326);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoy
                && case.startup_mode == StartupMode::SkipBoot
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
                && rom_path_without_store_prefix(&case.rom_path).starts_with("dmg")
        }));
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| matches!(case.pass_condition, PassCondition::MemoryBytesEqual(_)))
                .count(),
            1718
        );
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| matches!(
                    case.pass_condition,
                    PassCondition::FramebufferFixtureUntilMatch { .. }
                ))
                .count(),
            608
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "docboy-joypad-interactive-visual-joypad-buttons-02-joypad-buttons-a-png-inputs-0-a-pressed")
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| !case.external_stimuli.stimuli().is_empty())
        );
        let memory_case = suite
            .cases
            .iter()
            .find(|case| case.id == "docboy-cpu-cb-interrupt")
            .expect("DocBoy CPU memory row should exist");
        assert_eq!(
            memory_case.pass_condition,
            PassCondition::MemoryBytesEqual(vec![MemoryByteExpectation::with_fail_value(
                0xFFF0, 0x01, 0x02,
            )])
        );
        let exact_check_cases = suite
            .cases
            .iter()
            .filter_map(|case| {
                if let PassCondition::FramebufferFixtureUntilMatch {
                    check_at_tcycles: Some(check_at_tcycles),
                    ..
                } = &case.pass_condition
                {
                    Some((case.id.as_str(), *check_at_tcycles, case.timeout))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(exact_check_cases.len(), 6);
        for (case_id, check_at_tcycles, timeout) in exact_check_cases {
            let Timeout::TCycles(timeout_tcycles) = timeout else {
                panic!("{case_id} exact check_at case must use a T-cycle timeout");
            };
            assert!(
                timeout_tcycles >= check_at_tcycles,
                "{case_id} timeout {timeout_tcycles} must reach check_at_tcycles {check_at_tcycles}"
            );
        }
        assert!(suite_uses_docboy_test_report("docboy-dmg"));
        assert!(!suite_uses_extra_test_report("docboy-dmg"));
    }

    #[test]
    fn docboy_cgb_suite_tracks_native_cgb_docboy_rows() {
        let manifest_text = include_str!("../data/docboy/docboy-cgb.toml");
        assert!(
            !manifest_text.contains("startup ="),
            "DocBoy CGB manifest must stay startup-neutral so Make targets choose SkipBoot or RealBoot"
        );

        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "docboy-cgb")
            .expect("DocBoy CGB manifest should exist");
        assert_eq!(manifest.suite_family.as_deref(), Some("docboy-cgb"));
        assert_eq!(manifest.cases.len(), 6815);
        assert_eq!(
            manifest.cases.iter().filter(|case| case.disabled).count(),
            643
        );

        let suite = docboy_cgb_suite();

        assert_eq!(suite.name, "docboy-cgb");
        assert_eq!(suite.family.as_deref(), Some("docboy-cgb"));
        assert_eq!(suite.cases.len(), 6172);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
                && rom_path_without_store_prefix(&case.rom_path).starts_with("cgb")
        }));
        assert!(suite.cases.iter().all(|case| {
            !rom_path_without_store_prefix(&case.rom_path).starts_with("cgb/blargg/cgb_sound")
        }));
        assert!(suite.cases.iter().all(|case| {
            !rom_path_without_store_prefix(&case.rom_path).starts_with("cgb/samesuite")
        }));
        assert!(suite.cases.iter().all(|case| {
            !rom_path_without_store_prefix(&case.rom_path).starts_with("cgb/magen")
        }));
        assert!(suite.cases.iter().all(|case| {
            !rom_path_without_store_prefix(&case.rom_path).starts_with("cgb/daid")
        }));
        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.rom_path != Path::new("cgb/mattcurrie/cgb-acid2.gbc"))
        );
        assert!(
            suite
                .cases
                .iter()
                .all(|case| { case.rom_path != Path::new("cgb/little-things-gb/whichboot.gb") })
        );
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| matches!(case.pass_condition, PassCondition::MemoryBytesEqual(_)))
                .count(),
            6099
        );
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| matches!(
                    case.pass_condition,
                    PassCondition::FramebufferRgb555FixtureUntilMatch { .. }
                ))
                .count(),
            73
        );
        assert!(suite.cases.iter().any(|case| {
            case.id == "docboy-cgb-docboy-ppu-visual-stop-ly42-during-hblank-01-stop-ly42-during-hblank-a"
                && rom_path_without_store_prefix(&case.rom_path) == Path::new("cgb/ppu/visual/stop_ly42_during_hblank.gbc")
                && matches!(
                    case.pass_condition,
                    PassCondition::FramebufferRgb555FixtureUntilMatch {
                        check_at_tcycles: Some(4_739_304),
                        ..
                    }
                )
        }));
        assert!(suite.cases.iter().any(|case| {
            case.id
                == "docboy-cgb-docboy-double-speed-interactive-stop-key1-joypad0-interrupt1-ime0"
                && !case.external_stimuli.stimuli().is_empty()
        }));
        assert!(suite_uses_docboy_test_report("docboy-cgb"));
        assert!(!suite_uses_extra_test_report("docboy-cgb"));
    }

    #[test]
    fn docboy_cgb_dmg_suite_tracks_compatibility_mode_docboy_rows() {
        let manifest_text = include_str!("../data/docboy/docboy-cgb-dmg.toml");
        assert!(
            !manifest_text.contains("startup ="),
            "DocBoy CGB DMG manifest must stay startup-neutral so Make targets choose SkipBoot or RealBoot"
        );

        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "docboy-cgb-dmg")
            .expect("DocBoy CGB DMG manifest should exist");
        assert_eq!(manifest.suite_family.as_deref(), Some("docboy-cgb-dmg"));
        assert_eq!(manifest.cases.len(), 467);
        assert_eq!(
            manifest.cases.iter().filter(|case| case.disabled).count(),
            0
        );
        assert!(
            manifest
                .cases
                .iter()
                .all(|case| !case.rom.starts_with("mealybug")),
            "Mealybug rows should live in mealybug-tearoom-cgb-extra, not DocBoy"
        );
        assert!(manifest.cases.iter().all(|case| {
            !case.rom.starts_with(Path::new("mooneye/ppu"))
                && !case
                    .source_path
                    .starts_with(Path::new("tests/roms/cgb_dmg_mode/mooneye/ppu"))
        }));

        let suite = docboy_cgb_dmg_suite();

        assert_eq!(suite.name, "docboy-cgb-dmg");
        assert_eq!(suite.family.as_deref(), Some("docboy-cgb-dmg"));
        assert_eq!(suite.cases.len(), 467);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
                && rom_path_without_store_prefix(&case.rom_path).starts_with("cgb-dmg")
        }));
        let experimental_case_ids = [
            "docboy-cgb-dmg-mode-mode-cgb-flag-84",
            "docboy-cgb-dmg-mode-mode-cgb-flag-85",
            "docboy-cgb-dmg-mode-mode-cgb-flag-86",
            "docboy-cgb-dmg-mode-mode-cgb-flag-87",
        ];
        assert!(suite.cases.iter().all(|case| {
            let needs_experimental = experimental_case_ids.contains(&case.id.as_str());
            case.execution_mode
                == if needs_experimental {
                    crate::ExecutionMode::Experimental
                } else {
                    crate::ExecutionMode::Strict
                }
        }));
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| matches!(case.pass_condition, PassCondition::MemoryBytesEqual(_)))
                .count(),
            68
        );
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| matches!(
                    case.pass_condition,
                    PassCondition::FramebufferFixtureUntilMatch { .. }
                ))
                .count(),
            399
        );
        assert!(suite.cases.iter().all(|case| {
            case.id != "docboy-cgb-dmg-mooneye-boot-div-cgbabcde"
                && case.id != "docboy-cgb-dmg-mooneye-boot-regs-cgb"
        }));
        assert!(suite.cases.iter().all(|case| {
            !rom_path_without_store_prefix(&case.rom_path).starts_with("cgb-dmg/mealybug")
        }));
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| rom_path_without_store_prefix(&case.rom_path)
                    == Path::new("cgb-dmg/boot/boot_vram.gb"))
                .count(),
            1,
            "DocBoy cgb_dmg_mode.json carries one exact boot_vram duplicate that should not create duplicate runnable rows"
        );
        assert!(suite.cases.iter().any(|case| {
            case.id == "docboy-cgb-dmg-mode-mode-cgb-flag-84"
                && rom_path_without_store_prefix(&case.rom_path)
                    == Path::new("cgb-dmg/mode/mode_cgb_flag_84.gb")
                && case.pass_condition
                    == PassCondition::MemoryBytesEqual(vec![
                        MemoryByteExpectation::with_fail_value(0xFFF0, 0x01, 0x02),
                    ])
        }));
        assert!(suite_uses_docboy_test_report("docboy-cgb-dmg"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-dmg"));
    }

    #[test]
    fn mealybug_tearoom_cgb_extra_suite_owns_the_cgb_mealybug_rows() {
        let manifest_text = include_str!("../data/mealybug-tearoom-tests-cgb.toml");
        let custom_boot_case_ids = [
            "mealybug-cgb-m3-bgp-change-sprites",
            "mealybug-cgb-m3-lcdc-bg-map-change",
            "mealybug-cgb-m3-lcdc-obj-en-change",
            "mealybug-cgb-m3-lcdc-obj-en-change-variant",
            "mealybug-cgb-m3-lcdc-tile-sel-change",
            "mealybug-cgb-m3-obp0-change",
            "mealybug-cgb-m3-scx-low-3-bits",
        ];
        assert_eq!(
            manifest_text.matches("startup = \"custom-boot\"").count(),
            custom_boot_case_ids.len(),
            "Mealybug CGB extra rows should mirror the DMG manifest's custom-boot cases"
        );

        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "mealybug-tearoom-cgb-extra")
            .expect("Mealybug CGB extra manifest should exist");
        assert_eq!(
            manifest.suite_family.as_deref(),
            Some("mealybug-tearoom-tests")
        );
        assert_eq!(manifest.cases.len(), 24);
        assert!(manifest.cases.iter().all(|case| {
            case.family == "mealybug-tearoom-tests"
                && case.source_id == GBEMU_SHOOTOUT_SOURCE_ID
                && case
                    .source_path
                    .starts_with("testroms/mealybug-tearoom-tests/ppu")
                && case.report_console_suffix
                && case.console_model == ConsoleModel::GameBoyColor
                && !case.disabled
                && case.startup_mode
                    == if custom_boot_case_ids.contains(&case.id.as_str()) {
                        StartupMode::CustomBoot
                    } else {
                        StartupMode::SkipBoot
                    }
                && case.timeout == Timeout::Frames(30)
                && case.oracle == "framebuffer-rgb555-fixture"
        }));

        let suite = mealybug_tearoom_cgb_extra_suite();

        assert_eq!(suite.name, "mealybug-tearoom-cgb-extra");
        assert_eq!(suite.family.as_deref(), Some("mealybug-tearoom-tests"));
        assert_eq!(suite.cases.len(), 24);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode
                    == if custom_boot_case_ids.contains(&case.id.as_str()) {
                        StartupMode::CustomBoot
                    } else {
                        StartupMode::SkipBoot
                    }
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.timeout == Timeout::Frames(30)
                && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
                && rom_path_without_store_prefix(&case.rom_path)
                    .starts_with("mealybug-tearoom-tests/ppu")
                && matches!(
                    case.pass_condition,
                    PassCondition::FramebufferRgb555Fixture(_)
                )
        }));
        assert!(suite.cases.iter().any(|case| {
            case.id == "mealybug-cgb-m3-lcdc-win-en-change-multiple-wx"
                && matches!(
                    &case.pass_condition,
                    PassCondition::FramebufferRgb555Fixture(fixture_path)
                        if fixture_path
                            == Path::new(
                                "crates/gb-test-runner/data/mealybug-tearoom-tests/fixtures/cgb/m3_lcdc_win_en_change_multiple_wx.png"
                            )
                )
        }));
        assert!(suite_uses_extra_test_report("mealybug-tearoom-cgb-extra"));
        assert!(!suite_uses_docboy_test_report("mealybug-tearoom-cgb-extra"));
    }

    #[test]
    fn docboy_cgb_dmg_ext_suite_tracks_mixed_strict_and_experimental_docboy_rows() {
        let manifest_text = include_str!("../data/docboy/docboy-cgb-dmg-ext.toml");
        assert!(
            !manifest_text.contains("startup ="),
            "DocBoy CGB DMG-ext manifest must stay startup-neutral so Make targets choose SkipBoot or RealBoot"
        );

        let suite = docboy_cgb_dmg_ext_suite();

        assert_eq!(suite.name, "docboy-cgb-dmg-ext");
        assert_eq!(suite.family.as_deref(), Some("docboy-cgb-dmg-ext"));
        assert_eq!(suite.cases.len(), 26);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
                && rom_path_without_store_prefix(&case.rom_path).starts_with("cgb-dmg-ext")
        }));
        let experimental_case_ids = [
            "docboy-cgb-dmg-ext-apu-pcm12-ch2-read",
            "docboy-cgb-dmg-ext-hdma-gdma-basic-transfer",
            "docboy-cgb-dmg-ext-hdma-hdma-basic-transfer",
            "docboy-cgb-dmg-ext-ppu-ocpd-write-read",
        ];
        assert!(suite.cases.iter().all(|case| {
            let needs_experimental = experimental_case_ids.contains(&case.id.as_str());
            case.execution_mode
                == if needs_experimental {
                    crate::ExecutionMode::Experimental
                } else {
                    crate::ExecutionMode::Strict
                }
        }));
        assert!(suite.cases.iter().all(|case| {
            case.pass_condition
                == PassCondition::MemoryBytesEqual(vec![MemoryByteExpectation::with_fail_value(
                    0xFFF0, 0x01, 0x02,
                )])
        }));
        assert!(suite.cases.iter().any(|case| {
            case.id == "docboy-cgb-dmg-ext-mode-mode-cgb-flag-8c"
                && rom_path_without_store_prefix(&case.rom_path)
                    == Path::new("cgb-dmg-ext/mode/mode_cgb_flag_8c.gb")
        }));
        assert!(suite_uses_docboy_test_report("docboy-cgb-dmg-ext"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-dmg-ext"));
    }

    #[test]
    fn blargg_cgb_sound_suite_tracks_the_full_cgb_sound_lane() {
        let suite = blargg_cgb_sound_suite();

        assert_eq!(suite.name, "blargg-cgb-sound");
        assert_eq!(suite.family.as_deref(), Some("blargg"));
        assert_eq!(suite.cases.len(), 12);

        let expected = [
            (
                "blargg-cgb-sound-01-registers",
                "blargg/cgb_sound/01-registers.gb",
            ),
            (
                "blargg-cgb-sound-02-len-ctr",
                "blargg/cgb_sound/02-len_ctr.gb",
            ),
            (
                "blargg-cgb-sound-03-trigger",
                "blargg/cgb_sound/03-trigger.gb",
            ),
            ("blargg-cgb-sound-04-sweep", "blargg/cgb_sound/04-sweep.gb"),
            (
                "blargg-cgb-sound-05-sweep-details",
                "blargg/cgb_sound/05-sweep_details.gb",
            ),
            (
                "blargg-cgb-sound-06-overflow-on-trigger",
                "blargg/cgb_sound/06-overflow_on_trigger.gb",
            ),
            (
                "blargg-cgb-sound-07-len-sweep-period-sync",
                "blargg/cgb_sound/07-len_sweep_period_sync.gb",
            ),
            (
                "blargg-cgb-sound-08-len-ctr-during-power",
                "blargg/cgb_sound/08-len_ctr_during_power.gb",
            ),
            (
                "blargg-cgb-sound-09-wave-read-while-on",
                "blargg/cgb_sound/09-wave_read_while_on.gb",
            ),
            (
                "blargg-cgb-sound-10-wave-trigger-while-on",
                "blargg/cgb_sound/10-wave_trigger_while_on.gb",
            ),
            (
                "blargg-cgb-sound-11-regs-after-power",
                "blargg/cgb_sound/11-regs_after_power.gb",
            ),
            ("blargg-cgb-sound-12-wave", "blargg/cgb_sound/12-wave.gb"),
        ];

        for (case, (id, rom_path)) in suite.cases.iter().zip(expected) {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
            assert_eq!(
                rom_path_without_store_prefix(&case.rom_path),
                Path::new(rom_path)
            );
            assert_eq!(case.timeout, Timeout::Frames(3600));
            assert_eq!(
                case.pass_condition,
                PassCondition::MemoryTextOutputContains {
                    spec: blargg_memory_text_output_spec(),
                    expected_substring: "Passed".to_string(),
                }
            );
            assert!(case.capture_plan.contains(CaptureKind::MemoryTextOutput));
            assert!(case.capture_plan.contains(CaptureKind::Snapshot));
            assert!(
                case.failure_artifacts
                    .contains(CaptureKind::MemoryTextOutput)
            );
            assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
        }
    }

    #[test]
    fn samesuite_apu_suite_tracks_the_promoted_apu_lane() {
        let suite = samesuite_apu_suite();

        assert_eq!(suite.name, "samesuite-apu");
        assert_eq!(suite.family.as_deref(), Some("samesuite"));
        assert_eq!(suite.cases.len(), 61);

        let first = suite.cases.first().expect("suite should have cases");
        assert_eq!(first.id, "samesuite-apu-channel-1-channel-1-align");
        assert_eq!(
            rom_path_without_store_prefix(&first.rom_path),
            Path::new("samesuite/apu/channel_1/channel_1_align.gb")
        );

        let last = suite.cases.last().expect("suite should have cases");
        assert_eq!(last.id, "samesuite-apu-div-write-trigger-volume-10");
        assert_eq!(
            rom_path_without_store_prefix(&last.rom_path),
            Path::new("samesuite/apu/div_write_trigger_volume_10.gb")
        );
        assert_eq!(last.timeout, Timeout::Frames(180));

        for case in &suite.cases {
            assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
            assert!(
                rom_path_without_store_prefix(&case.rom_path).starts_with("samesuite/apu"),
                "{} should point at SameSuite APU",
                case.rom_path.display()
            );
            assert!(matches!(
                case.pass_condition,
                PassCondition::FramebufferRgb555Fixture(_)
            ));
            assert!(case.capture_plan.contains(CaptureKind::Framebuffer));
            assert!(case.capture_plan.contains(CaptureKind::Snapshot));
            assert!(case.failure_artifacts.contains(CaptureKind::Framebuffer));
            assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
        }
    }

    #[test]
    fn samesuite_cgb_extra_suite_tracks_docboy_sourced_cgb_variants() {
        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "samesuite-cgb-extra")
            .expect("SameSuite CGB manifest should exist");
        assert_eq!(manifest.suite_family.as_deref(), Some("samesuite"));
        assert_eq!(manifest.cases.len(), 10);
        assert!(manifest.cases.iter().all(|case| {
            case.source_id == "docboy"
                && case.source_path.starts_with("tests/roms/cgb/samesuite")
                && case.report_console_suffix
        }));
        let disabled_sweep_restart_2 = manifest
            .cases
            .iter()
            .find(|case| case.id == "samesuite-cgb-apu-channel-1-channel-1-sweep-restart-2-cgbe")
            .expect("DocBoy CGB-E sweep-restart-2 variant should stay tracked");
        assert!(disabled_sweep_restart_2.disabled);
        assert_eq!(
            disabled_sweep_restart_2.oracle,
            "framebuffer-rgb555-fixture"
        );
        assert!(
            disabled_sweep_restart_2
                .comment
                .as_deref()
                .is_some_and(|comment| comment.contains("promoted public GBEmulatorShootout"))
        );

        let suite = samesuite_cgb_extra_suite();

        assert_eq!(suite.name, "samesuite-cgb-extra");
        assert_eq!(suite.family.as_deref(), Some("samesuite"));
        assert_eq!(suite.cases.len(), 9);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.timeout == Timeout::Frames(180)
                && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
                && rom_path_without_store_prefix(&case.rom_path).starts_with("samesuite/apu")
                && matches!(
                    case.pass_condition,
                    PassCondition::FramebufferRgb555Fixture(_)
                )
        }));
        assert!(!suite
            .cases
            .iter()
            .any(|case| case.id == "samesuite-cgb-apu-channel-1-channel-1-sweep-restart-2-cgbe"));
        assert!(suite.cases.iter().any(|case| {
            case.id == "samesuite-cgb-apu-channel-3-channel-3-wave-ram-dac-on-rw"
                && rom_path_without_store_prefix(&case.rom_path)
                    == Path::new("samesuite/apu/channel_3/channel_3_wave_ram_dac_on_rw.gb")
        }));
        assert!(suite_uses_extra_test_report("samesuite-cgb-extra"));
        assert!(!suite_uses_docboy_test_report("samesuite-cgb-extra"));
    }

    #[test]
    fn magen_cgb_extra_suite_tracks_docboy_sourced_cgb_rows() {
        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "magen-cgb-extra")
            .expect("Magen CGB manifest should exist");
        assert_eq!(manifest.suite_family.as_deref(), Some("magen"));
        assert_eq!(manifest.cases.len(), 8);
        assert!(manifest.cases.iter().all(|case| {
            case.source_id == "docboy"
                && case.source_path.starts_with("tests/roms/cgb/magen")
                && !case.report_console_suffix
        }));

        let suite = magen_cgb_extra_suite();

        assert_eq!(suite.name, "magen-cgb-extra");
        assert_eq!(suite.family.as_deref(), Some("magen"));
        assert_eq!(suite.cases.len(), 8);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.timeout == Timeout::TCycles(5_000_000)
                && case.rom_path.starts_with(Path::new(TEST_ROM_STORE_DIR))
                && rom_path_without_store_prefix(&case.rom_path).starts_with("magen")
                && matches!(
                    case.pass_condition,
                    PassCondition::FramebufferRgb555Fixture(_)
                )
        }));
        assert!(suite.cases.iter().any(|case| {
            case.id == "magen-cgb-bg-oam-priority"
                && rom_path_without_store_prefix(&case.rom_path)
                    == Path::new("magen/bg_oam_priority.gbc")
        }));
        assert!(suite_uses_extra_test_report("magen-cgb-extra"));
        assert!(!suite_uses_docboy_test_report("magen-cgb-extra"));
    }

    #[test]
    fn manifests_mark_current_gbemu_shootout_console_suffixed_rows() {
        let dmg_rows = [
            ("acid", "which.gb"),
            ("daid", "ppu_scanline_bgp.gb"),
            ("daid", "stop_instr.gb"),
            ("ashiepaws", "bully.gb"),
            ("mealybug-tearoom-tests", "ppu/m2_win_en_toggle.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_bgp_change.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_bgp_change_sprites.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_lcdc_bg_en_change.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_lcdc_bg_map_change.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_lcdc_obj_en_change.gb"),
            (
                "mealybug-tearoom-tests",
                "ppu/m3_lcdc_obj_en_change_variant.gb",
            ),
            ("mealybug-tearoom-tests", "ppu/m3_lcdc_obj_size_change.gb"),
            (
                "mealybug-tearoom-tests",
                "ppu/m3_lcdc_obj_size_change_scx.gb",
            ),
            ("mealybug-tearoom-tests", "ppu/m3_lcdc_tile_sel_change.gb"),
            (
                "mealybug-tearoom-tests",
                "ppu/m3_lcdc_tile_sel_win_change.gb",
            ),
            (
                "mealybug-tearoom-tests",
                "ppu/m3_lcdc_win_en_change_multiple.gb",
            ),
            (
                "mealybug-tearoom-tests",
                "ppu/m3_lcdc_win_en_change_multiple_wx.gb",
            ),
            ("mealybug-tearoom-tests", "ppu/m3_lcdc_win_map_change.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_obp0_change.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_scx_high_5_bits.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_scx_low_3_bits.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_scy_change.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_window_timing.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_window_timing_wx_0.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_wx_4_change.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_wx_4_change_sprites.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_wx_5_change.gb"),
            ("mealybug-tearoom-tests", "ppu/m3_wx_6_change.gb"),
        ];
        let manifests = curated_test_rom_manifests();

        for (family, rom) in dmg_rows {
            let case = manifests
                .iter()
                .flat_map(|manifest| &manifest.cases)
                .find(|case| {
                    case.family == family
                        && case.rom == Path::new(rom)
                        && case.console_model == ConsoleModel::GameBoy
                })
                .unwrap_or_else(|| panic!("missing GBEmulatorShootout DMG row {family}/{rom}"));

            assert_eq!(case.console_model, ConsoleModel::GameBoy, "{family}/{rom}");
            assert!(case.report_console_suffix, "{family}/{rom}");
            assert_eq!(
                manifest_case_report_rom_display(case),
                format!("{rom} (DMG)")
            );
        }

        let cgb_which = manifests
            .iter()
            .flat_map(|manifest| &manifest.cases)
            .find(|case| {
                case.family == "acid"
                    && case.rom == Path::new("which.gb")
                    && case.console_model == ConsoleModel::GameBoyColor
            })
            .expect("CGB Acid which row should exist");
        assert!(cgb_which.report_console_suffix);
        assert_eq!(
            manifest_case_report_rom_display(cgb_which),
            "which.gb (GBC)"
        );

        let cgb_scanline_bgp = manifests
            .iter()
            .flat_map(|manifest| &manifest.cases)
            .find(|case| {
                case.family == "daid"
                    && case.rom == Path::new("ppu_scanline_bgp.gb")
                    && case.console_model == ConsoleModel::GameBoyColor
            })
            .expect("CGB Daid ppu_scanline_bgp row should exist");
        assert!(cgb_scanline_bgp.report_console_suffix);
        assert_eq!(
            manifest_case_report_rom_display(cgb_scanline_bgp),
            "ppu_scanline_bgp.gb (GBC)"
        );

        let cgb_bully = manifests
            .iter()
            .flat_map(|manifest| &manifest.cases)
            .find(|case| {
                case.family == "ashiepaws"
                    && case.rom == Path::new("bully.gb")
                    && case.console_model == ConsoleModel::GameBoyColor
            })
            .expect("CGB Ashiepaws bully row should exist");
        assert!(cgb_bully.report_console_suffix);
        assert_eq!(
            manifest_case_report_rom_display(cgb_bully),
            "bully.gb (GBC)"
        );

        let cgb_boot_regs = manifests
            .iter()
            .flat_map(|manifest| &manifest.cases)
            .find(|case| case.family == "mooneye" && case.rom == Path::new("misc/boot_regs-cgb.gb"))
            .expect("CGB Mooneye boot_regs row should exist");
        assert!(!cgb_boot_regs.report_console_suffix);
        assert_eq!(
            manifest_case_report_rom_display(cgb_boot_regs),
            "misc/boot_regs-cgb.gb"
        );
    }

    #[test]
    fn ashiepaws_manifest_uses_ashiepaws_upstream_paths_and_family() {
        let manifests = curated_test_rom_manifests();
        let ashiepaws_cases = manifests
            .iter()
            .flat_map(|manifest| &manifest.cases)
            .filter(|case| case.family == "ashiepaws")
            .map(|case| {
                (
                    case.id.as_str(),
                    case.rom.as_path(),
                    case.source_path.as_path(),
                )
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(
            ashiepaws_cases,
            BTreeSet::from([
                (
                    "ashiepaws-bully-cgb",
                    Path::new("bully.gb"),
                    Path::new("testroms/ashiepaws/bully.gb")
                ),
                (
                    "ashiepaws-bully-dmg",
                    Path::new("bully.gb"),
                    Path::new("testroms/ashiepaws/bully.gb")
                ),
                (
                    "ashiepaws-strikethrough",
                    Path::new("strikethrough.gb"),
                    Path::new("testroms/ashiepaws/strikethrough.gb")
                ),
            ])
        );
    }

    #[test]
    fn curated_family_suite_builders_preserve_each_supported_oracle_shape() {
        let suites = curated_test_rom_family_suites();

        assert_eq!(suites.len(), 11);
        assert!(suites.iter().any(|suite| suite.name == "acid"));
        assert!(suites.iter().any(|suite| suite.name == "blargg-cpu-instrs"));
        assert!(suites.iter().any(|suite| suite.name == "blargg-dmg-sound"));
        assert!(
            suites
                .iter()
                .any(|suite| suite.name == "blargg-timing-memory-oam")
        );
        assert!(suites.iter().any(|suite| suite.name == "cpp"));
        assert!(suites.iter().any(|suite| suite.name == "daid"));
        assert!(suites.iter().any(|suite| suite.name == "ashiepaws"));
        assert!(
            suites
                .iter()
                .any(|suite| suite.name == "mealybug-tearoom-tests")
        );
        assert!(
            suites
                .iter()
                .any(|suite| suite.name == "mooneye-acceptance-manual-misc")
        );
        assert!(
            suites
                .iter()
                .any(|suite| suite.name == "mooneye-emulator-mbc1-mbc5")
        );
        assert!(
            suites
                .iter()
                .any(|suite| suite.name == "mooneye-emulator-mbc2")
        );

        let acid_suite = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("acid"))
            .expect("acid suite should exist");
        let acid_info_case = acid_suite
            .cases
            .iter()
            .find(|case| case.id == "acid-which-dmg")
            .expect("acid DMG informational case should exist");
        assert!(matches!(
            acid_info_case.pass_condition,
            PassCondition::Informational(InformationalCaptureKind::Framebuffer)
        ));
        assert!(
            acid_info_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert!(acid_info_case.capture_plan.contains(CaptureKind::Snapshot));

        let acid_cgb_info_case = acid_suite
            .cases
            .iter()
            .find(|case| case.id == "acid-which-cgb")
            .expect("acid CGB informational case should exist");
        assert_eq!(acid_cgb_info_case.console_model, ConsoleModel::GameBoyColor);
        assert!(matches!(
            acid_cgb_info_case.pass_condition,
            PassCondition::Informational(InformationalCaptureKind::Framebuffer)
        ));
        assert!(
            acid_cgb_info_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert!(
            acid_cgb_info_case
                .capture_plan
                .contains(CaptureKind::Snapshot)
        );

        let acid_case = acid_suite
            .cases
            .iter()
            .find(|case| case.id == "acid-dmg-acid2")
            .expect("acid framebuffer fixture case should exist");
        assert!(matches!(
            acid_case.pass_condition,
            PassCondition::FramebufferFixture(_)
        ));
        assert!(acid_case.capture_plan.contains(CaptureKind::Framebuffer));
        assert!(acid_case.capture_plan.contains(CaptureKind::Snapshot));

        let cgb_acid2_case = acid_suite
            .cases
            .iter()
            .find(|case| case.id == "acid-cgb-acid2")
            .expect("acid CGB Acid2 framebuffer fixture case should exist");
        assert_eq!(cgb_acid2_case.console_model, ConsoleModel::GameBoyColor);
        assert!(matches!(
            cgb_acid2_case.pass_condition,
            PassCondition::FramebufferRgb555GrayscaleToleranceFixture(_)
        ));
        assert!(
            cgb_acid2_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert!(cgb_acid2_case.capture_plan.contains(CaptureKind::Snapshot));

        let acid_hell_case = acid_suite
            .cases
            .iter()
            .find(|case| case.id == "acid-cgb-acid-hell")
            .expect("acid CGB Hell framebuffer fixture case should exist");
        assert_eq!(acid_hell_case.console_model, ConsoleModel::GameBoyColor);
        assert!(matches!(
            acid_hell_case.pass_condition,
            PassCondition::FramebufferRgb555Fixture(_)
        ));
        assert!(
            acid_hell_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert!(acid_hell_case.capture_plan.contains(CaptureKind::Snapshot));

        let cpp_suite = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("cpp"))
            .expect("cpp suite should exist");
        assert_eq!(cpp_suite.cases.len(), 4);
        assert!(
            cpp_suite
                .cases
                .iter()
                .all(|case| matches!(case.pass_condition, PassCondition::FramebufferFixture(_)))
        );

        let halt_bug_case = suites
            .iter()
            .filter(|suite| suite.family.as_deref() == Some("blargg"))
            .flat_map(|suite| suite.cases.iter())
            .find(|case| case.id == "blargg-halt-bug")
            .expect("halt bug case should exist");
        assert!(matches!(
            halt_bug_case.pass_condition,
            PassCondition::BlarggConsoleTextContains(_)
        ));
        assert!(
            halt_bug_case
                .capture_plan
                .contains(CaptureKind::BlarggConsoleText)
        );

        let mealybug_case = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("mealybug-tearoom-tests"))
            .and_then(|suite| {
                suite
                    .cases
                    .iter()
                    .find(|case| case.id == "mealybug-tearoom-tests-ppu-m3-bgp-change-sprites")
            })
            .expect("mealybug custom-boot case should exist");
        assert_eq!(mealybug_case.startup_mode, StartupMode::CustomBoot);
        assert!(mealybug_case.startup_memory_writes.is_empty());

        let ashiepaws_case = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("ashiepaws"))
            .and_then(|suite| {
                suite
                    .cases
                    .iter()
                    .find(|case| case.id == "ashiepaws-bully-dmg")
            })
            .expect("ashiepaws skip-boot case should exist");
        assert_eq!(ashiepaws_case.startup_mode, StartupMode::SkipBoot);
        assert!(ashiepaws_case.startup_memory_writes.is_empty());

        let strikethrough_case = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("ashiepaws"))
            .and_then(|suite| {
                suite
                    .cases
                    .iter()
                    .find(|case| case.id == "ashiepaws-strikethrough")
            })
            .expect("ashiepaws framebuffer case should exist");
        assert!(matches!(
            strikethrough_case.pass_condition,
            PassCondition::FramebufferFixture(_)
        ));
        assert!(
            strikethrough_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert!(
            strikethrough_case
                .capture_plan
                .contains(CaptureKind::Snapshot)
        );

        let mooneye_case = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("mooneye"))
            .and_then(|suite| suite.cases.first())
            .expect("mooneye case should exist");
        assert!(matches!(
            mooneye_case.pass_condition,
            PassCondition::MooneyeResult
        ));
        assert_eq!(
            mooneye_case.capture_plan,
            crate::CapturePlan::new()
                .with_capture(CaptureKind::Snapshot)
                .with_capture(CaptureKind::Serial)
        );
    }

    #[test]
    fn curated_test_rom_families_report_the_supported_sorted_family_ids() {
        assert_eq!(
            curated_test_rom_families(),
            vec![
                "acid".to_string(),
                "ashiepaws".to_string(),
                "ax6".to_string(),
                "blargg".to_string(),
                "cpp".to_string(),
                "daid".to_string(),
                "docboy-cgb".to_string(),
                "docboy-cgb-dmg".to_string(),
                "docboy-cgb-dmg-ext".to_string(),
                "docboy-dmg".to_string(),
                "gbmicrotest".to_string(),
                "little-things-gb".to_string(),
                "magen".to_string(),
                "mealybug-tearoom-tests".to_string(),
                "mooneye".to_string(),
                "samesuite".to_string(),
            ]
        );
    }

    #[test]
    fn curated_test_rom_families_without_a_report_are_legacy_only() {
        assert_eq!(
            curated_test_rom_families_for_report(None)
                .expect("legacy family selection should resolve"),
            vec![
                "ax6".to_string(),
                "little-things-gb".to_string(),
                "magen".to_string(),
                "mealybug-tearoom-tests".to_string(),
                "mooneye".to_string(),
                "samesuite".to_string(),
            ]
        );
    }

    #[test]
    fn curated_test_rom_families_can_be_limited_to_the_gb_emulator_shootout_report() {
        assert_eq!(
            curated_test_rom_families_for_report(Some(GB_EMULATOR_SHOOTOUT_REPORT_ID))
                .expect("GB Emulator Shootout report families should resolve"),
            vec![
                "acid".to_string(),
                "ashiepaws".to_string(),
                "ax6".to_string(),
                "blargg".to_string(),
                "cpp".to_string(),
                "daid".to_string(),
                "mealybug-tearoom-tests".to_string(),
                "mooneye".to_string(),
                "samesuite".to_string(),
            ]
        );
        assert!(
            curated_test_rom_families_for_report(Some("unknown-report"))
                .expect_err("unknown report should be rejected")
                .contains("unknown curated test ROM report")
        );

        let standard_suite = samesuite_suite();
        assert_eq!(
            standard_suite.report_id.as_deref(),
            Some(GB_EMULATOR_SHOOTOUT_REPORT_ID)
        );
        assert!(
            standard_suite
                .cases
                .iter()
                .all(|case| case.report_id.as_deref() == Some(GB_EMULATOR_SHOOTOUT_REPORT_ID))
        );
        assert_eq!(mooneye_cgb_extra_suite().report_id, None);
        assert_eq!(
            docboy_cgb_suite().report_id.as_deref(),
            Some(DOCBOY_REPORT_ID)
        );
        assert_eq!(
            gbmicrotest_suite().report_id.as_deref(),
            Some(GBMICROTEST_REPORT_ID)
        );
    }

    #[test]
    fn curated_test_rom_families_can_be_limited_to_the_docboy_report() {
        assert_eq!(
            curated_test_rom_families_for_report(Some(DOCBOY_REPORT_ID))
                .expect("DocBoy report families should resolve"),
            vec![
                "docboy-cgb".to_string(),
                "docboy-cgb-dmg".to_string(),
                "docboy-cgb-dmg-ext".to_string(),
                "docboy-dmg".to_string(),
            ]
        );
        let docboy_suite = docboy_cgb_suite();
        assert_eq!(docboy_suite.report_id.as_deref(), Some(DOCBOY_REPORT_ID));
        assert!(
            docboy_suite
                .cases
                .iter()
                .all(|case| case.report_id.as_deref() == Some(DOCBOY_REPORT_ID))
        );
    }

    #[test]
    fn curated_test_rom_families_can_be_limited_to_the_gbmicrotest_report() {
        assert_eq!(
            curated_test_rom_families_for_report(Some(GBMICROTEST_REPORT_ID))
                .expect("gbmicrotest report families should resolve"),
            vec!["gbmicrotest".to_string()]
        );
        let suite = gbmicrotest_suite();
        assert_eq!(suite.report_id.as_deref(), Some(GBMICROTEST_REPORT_ID));
        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.report_id.as_deref() == Some(GBMICROTEST_REPORT_ID))
        );
    }

    #[test]
    fn docboy_report_uses_report_local_sources_and_committed_fixtures() {
        let parsed: CuratedSourceManifestFile =
            toml::from_str(curated_source_manifest_text(Some(DOCBOY_REPORT_ID)))
                .expect("DocBoy source manifest should parse");
        let allowed_families = BTreeSet::from([
            "docboy-dmg",
            "docboy-cgb",
            "docboy-cgb-dmg",
            "docboy-cgb-dmg-ext",
        ]);
        let required_files = parsed
            .sources
            .into_iter()
            .flat_map(|source| source.required_files)
            .collect::<Vec<_>>();

        assert!(!required_files.is_empty());
        assert!(required_files.iter().all(|file| {
            file.family
                .as_deref()
                .is_some_and(|family| allowed_families.contains(family))
        }));
        assert!(
            required_files
                .iter()
                .any(|file| { file.path.starts_with(Path::new("tests/results/dmg/docboy")) })
        );
        assert!(required_files.iter().all(|file| {
            let path = file.path.to_string_lossy();
            !path.contains("serial_two_players_basic_transfer") && !path.ends_with("ok.png")
        }));

        let fixture_prefix = Path::new("crates/gb-test-runner/data/docboy/fixtures");
        let fixture_paths = curated_test_rom_manifests()
            .into_iter()
            .filter(|manifest| suite_report_id(&manifest.suite_name) == Some(DOCBOY_REPORT_ID))
            .flat_map(|manifest| manifest.cases)
            .filter(|case| !case.disabled)
            .flat_map(|case| {
                case.fixture
                    .into_iter()
                    .flat_map(ManifestFixtureField::into_paths)
            })
            .collect::<Vec<_>>();

        assert!(!fixture_paths.is_empty());
        assert!(
            fixture_paths
                .iter()
                .all(|path| path.starts_with(fixture_prefix))
        );
    }

    #[test]
    fn gbmicrotest_report_uses_report_local_sources_without_case_source_fields() {
        let parsed: CuratedSourceManifestFile =
            toml::from_str(curated_source_manifest_text(Some(GBMICROTEST_REPORT_ID)))
                .expect("gbmicrotest source manifest should parse");
        assert_eq!(parsed.sources.len(), 1);
        assert_eq!(parsed.sources[0].id, "gbmicrotest");
        assert_eq!(parsed.sources[0].required_files.len(), 438);
        assert!(parsed.sources[0].required_files.iter().all(|file| {
            file.family.as_deref() == Some("gbmicrotest")
                && file.rom.is_some()
                && file
                    .path
                    .starts_with(Path::new("tests/roms/dmg/gbmicrotest"))
        }));

        let manifest: CuratedTestRomManifestFile =
            toml::from_str(include_str!("../data/gbmicrotest/gbmicrotest.toml"))
                .expect("gbmicrotest manifest should parse");
        assert_eq!(manifest.suite_name, "gbmicrotest");
        assert_eq!(manifest.family.as_deref(), Some("gbmicrotest"));
        assert_eq!(
            manifest.defaults.oracle.as_deref(),
            Some("memory-byte-equals")
        );
        assert_eq!(manifest.defaults.console.as_deref(), Some("dmg"));
        assert_eq!(manifest.defaults.timeout_frames, Some(15));
        assert_eq!(manifest.defaults.memory.as_ref().map(Vec::len), Some(1));
        assert!(manifest.cases.iter().all(|case| {
            case.source_id.is_none()
                && case.source_path.is_none()
                && case.oracle.is_none()
                && case.memory.is_none()
                && case.timeout_tcycles.is_none()
                && (case.timeout_frames.is_none() || case.timeout_frames == Some(30))
        }));
    }

    #[test]
    fn gb_emulator_shootout_keeps_only_manual_sgb_fixtures_committed() {
        let fixtures_root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/gb-emulator-shootout/fixtures");
        let mut fixtures = Vec::new();
        collect_fixture_files(&fixtures_root, &fixtures_root, &mut fixtures);
        fixtures.sort();

        assert_eq!(
            fixtures,
            vec![
                PathBuf::from("cpp/sgb-ext-test.sgb.png"),
                PathBuf::from("samesuite/sgb/command_mlt_req.png"),
                PathBuf::from("samesuite/sgb/command_mlt_req_1_incrementing.png"),
            ]
        );
    }

    #[test]
    fn gb_emulator_shootout_source_manifest_covers_report_store_fixtures() {
        let parsed: CuratedSourceManifestFile = toml::from_str(curated_source_manifest_text(Some(
            GB_EMULATOR_SHOOTOUT_REPORT_ID,
        )))
        .expect("GB Emulator Shootout source manifest should parse");
        let source_targets = parsed
            .sources
            .into_iter()
            .flat_map(|source| source.required_files)
            .filter_map(|file| {
                let family = file
                    .family
                    .or_else(|| required_file_family(&file.path).map(str::to_string))?;
                let target = file.target?;
                Some((family, target))
            })
            .collect::<BTreeSet<_>>();
        let store_prefix = Path::new(TEST_ROM_STORE_DIR).join(GB_EMULATOR_SHOOTOUT_REPORT_ID);
        let committed_fixture_prefix =
            Path::new("crates/gb-test-runner/data/gb-emulator-shootout/fixtures");
        let mut store_fixtures = BTreeSet::new();
        let mut committed_fixtures = BTreeSet::new();

        for manifest in curated_test_rom_manifests().into_iter().filter(|manifest| {
            suite_report_id(&manifest.suite_name) == Some(GB_EMULATOR_SHOOTOUT_REPORT_ID)
        }) {
            for case in manifest.cases {
                for fixture_path in case
                    .fixture
                    .into_iter()
                    .flat_map(ManifestFixtureField::into_paths)
                {
                    if let Ok(target) = fixture_path.strip_prefix(&store_prefix) {
                        let mut components = target.components();
                        let family = components
                            .next()
                            .expect("report-store fixture should include family")
                            .as_os_str()
                            .to_string_lossy()
                            .into_owned();
                        store_fixtures.insert((family, components.collect::<PathBuf>()));
                    } else if fixture_path.starts_with(committed_fixture_prefix) {
                        committed_fixtures.insert(
                            fixture_path
                                .strip_prefix(committed_fixture_prefix)
                                .expect("fixture should strip committed prefix")
                                .to_path_buf(),
                        );
                    }
                }
            }
        }

        assert!(!store_fixtures.is_empty());
        for (family, fixture) in &store_fixtures {
            assert!(
                source_targets.contains(&(family.clone(), fixture.clone())),
                "missing report source target for fixture {}/{}",
                family,
                fixture.display()
            );
        }
        assert_eq!(
            committed_fixtures,
            BTreeSet::from([
                PathBuf::from("cpp/sgb-ext-test.sgb.png"),
                PathBuf::from("samesuite/sgb/command_mlt_req.png"),
                PathBuf::from("samesuite/sgb/command_mlt_req_1_incrementing.png"),
            ])
        );
    }

    #[test]
    fn discover_test_rom_store_root_prefers_env_then_existing_default_then_none() {
        let workspace_root = unique_temp_dir("discover-root");
        let default_root = test_rom_store_root(&workspace_root);
        fs::create_dir_all(&default_root).expect("default test ROM store should be creatable");

        let _guard = crate::test_support::lock_env();
        let previous = env::var_os(TEST_ROM_ROOT_ENV_VAR);
        remove_env_var(TEST_ROM_ROOT_ENV_VAR);

        let discovered_default = discover_test_rom_store_root(&workspace_root);
        assert_eq!(discovered_default, Some(default_root.clone()));

        let env_root = workspace_root.join("custom-test-root");
        set_env_var(TEST_ROM_ROOT_ENV_VAR, &env_root);
        let discovered_env = discover_test_rom_store_root(&workspace_root);
        assert_eq!(discovered_env, Some(env_root.clone()));

        remove_env_var(TEST_ROM_ROOT_ENV_VAR);
        fs::remove_dir_all(&default_root).expect("default test ROM store should be removable");
        assert_eq!(discover_test_rom_store_root(&workspace_root), None);

        match previous {
            Some(value) => set_env_var(TEST_ROM_ROOT_ENV_VAR, value),
            None => remove_env_var(TEST_ROM_ROOT_ENV_VAR),
        }
        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn discover_test_rom_store_root_for_report_treats_env_as_global_root() {
        let workspace_root = unique_temp_dir("discover-report-root");
        let default_report_root =
            test_rom_store_root_for_report(&workspace_root, GB_EMULATOR_SHOOTOUT_REPORT_ID);
        fs::create_dir_all(&default_report_root)
            .expect("default report test ROM store should be creatable");

        let _guard = crate::test_support::lock_env();
        let previous = env::var_os(TEST_ROM_ROOT_ENV_VAR);
        remove_env_var(TEST_ROM_ROOT_ENV_VAR);

        assert_eq!(
            super::discover_test_rom_store_root_for_report(
                &workspace_root,
                GB_EMULATOR_SHOOTOUT_REPORT_ID
            ),
            Some(default_report_root.clone())
        );

        let env_root = workspace_root.join("custom-test-root");
        set_env_var(TEST_ROM_ROOT_ENV_VAR, &env_root);
        assert_eq!(
            super::discover_test_rom_store_root_for_report(
                &workspace_root,
                GB_EMULATOR_SHOOTOUT_REPORT_ID
            ),
            Some(env_root.join(GB_EMULATOR_SHOOTOUT_REPORT_ID))
        );

        remove_env_var(TEST_ROM_ROOT_ENV_VAR);
        fs::remove_dir_all(&default_report_root)
            .expect("default report test ROM store should be removable");
        assert_eq!(
            super::discover_test_rom_store_root_for_report(
                &workspace_root,
                GB_EMULATOR_SHOOTOUT_REPORT_ID
            ),
            None
        );

        match previous {
            Some(value) => set_env_var(TEST_ROM_ROOT_ENV_VAR, value),
            None => remove_env_var(TEST_ROM_ROOT_ENV_VAR),
        }
        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn materialize_curated_store_copies_roms_and_replaces_existing_family_dirs() {
        let workspace_root = unique_temp_dir("materialize");
        let gbemu_shootout_root = workspace_root.join("gbemu-shootout");
        write_fake_gbemu_shootout_tree(&gbemu_shootout_root);

        let stale_family_root = test_rom_store_root(&workspace_root).join("ax6");
        fs::create_dir_all(&stale_family_root).expect("stale family root should be creatable");
        fs::write(stale_family_root.join("stale.txt"), "old").expect("stale file should write");

        materialize_curated_test_rom_store(&workspace_root, &gbemu_shootout_root)
            .expect("curated test ROM store should materialize");

        assert!(!stale_family_root.join("stale.txt").exists());
        for manifest in curated_test_rom_manifests() {
            if super::suite_report_id(&manifest.suite_name).is_some() {
                continue;
            }
            for case in manifest
                .cases
                .iter()
                .filter(|case| case.source_id == GBEMU_SHOOTOUT_SOURCE_ID)
            {
                let family_root = test_rom_store_root(&workspace_root).join(&case.family);
                assert!(!family_root.join("catalog.toml").exists());
                assert_eq!(
                    fs::read_to_string(family_root.join(&case.rom))
                        .expect("curated ROM should be readable"),
                    format!("{}:{}", case.family, case.rom.display())
                );
            }
        }
        assert_eq!(
            fs::read_to_string(
                test_rom_store_root(&workspace_root).join("mooneye/misc/boot_hwio-C.gb")
            )
            .expect("CGB boot HWIO ROM should be materialized from the legacy manifest"),
            "mooneye:misc/boot_hwio-C.gb"
        );
        assert!(!test_rom_store_root(&workspace_root).join("acid").exists());
        assert!(!test_rom_store_root(&workspace_root).join("blargg").exists());
        assert!(
            !test_rom_store_root(&workspace_root)
                .join("samesuite/interrupt/ei_delay_halt.gb")
                .exists()
        );
        assert!(
            !test_rom_store_root(&workspace_root)
                .join("little-things-gb")
                .exists()
        );

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn materialize_curated_store_reports_missing_source_roms() {
        let workspace_root = unique_temp_dir("materialize-missing");
        let gbemu_shootout_root = workspace_root.join("gbemu-shootout");
        fs::create_dir_all(&gbemu_shootout_root).expect("fake source root should be creatable");

        let error = materialize_curated_test_rom_store(&workspace_root, &gbemu_shootout_root)
            .expect_err("missing curated ROM source should fail");

        assert!(error.contains("failed to copy curated ROM"));
        assert!(error.contains("testroms/ax6/"));

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn materialize_curated_store_can_limit_selected_families_without_touching_others() {
        let workspace_root = unique_temp_dir("materialize-selected");
        let gbemu_shootout_root = workspace_root.join("gbemu-shootout");
        write_fake_gbemu_shootout_tree(&gbemu_shootout_root);

        let store_root = test_rom_store_root(&workspace_root);
        let ax6_root = store_root.join("ax6");
        let mooneye_root = store_root.join("mooneye");
        fs::create_dir_all(&ax6_root).expect("AX6 family root should be creatable");
        fs::create_dir_all(&mooneye_root).expect("mooneye family root should be creatable");
        fs::write(ax6_root.join("stale.txt"), "replace").expect("AX6 stale file should write");
        fs::write(mooneye_root.join("keep.txt"), "keep").expect("mooneye marker should write");

        materialize_curated_test_rom_families(
            &workspace_root,
            &gbemu_shootout_root,
            &["ax6".to_string()],
        )
        .expect("selected curated families should materialize");

        assert!(!ax6_root.join("stale.txt").exists());
        assert!(ax6_root.join("rtc3test-1.gb").exists());
        assert!(ax6_root.join("rtc3test-2.gb").exists());
        assert!(mooneye_root.join("keep.txt").exists());
        assert!(!mooneye_root.join("acceptance").exists());

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn materialize_curated_store_rejects_unknown_selected_families() {
        let workspace_root = unique_temp_dir("materialize-selected-unknown");
        let gbemu_shootout_root = workspace_root.join("gbemu-shootout");
        write_fake_gbemu_shootout_tree(&gbemu_shootout_root);

        let error = materialize_curated_test_rom_families(
            &workspace_root,
            &gbemu_shootout_root,
            &["unknown".to_string()],
        )
        .expect_err("unknown curated families should be rejected");
        assert!(error.contains("unknown curated test ROM family selection"));
        assert!(error.contains("unknown"));

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn materialize_curated_store_rejects_docboy_only_selected_families() {
        let workspace_root = unique_temp_dir("materialize-selected-docboy-only");
        let gbemu_shootout_root = workspace_root.join("gbemu-shootout");
        write_fake_gbemu_shootout_tree(&gbemu_shootout_root);

        let error = materialize_curated_test_rom_families(
            &workspace_root,
            &gbemu_shootout_root,
            &["little-things-gb".to_string()],
        )
        .expect_err("single-source GBEmulator materialization should reject DocBoy-only families");
        assert!(error.contains("unknown curated test ROM family selection"));
        assert!(error.contains("little-things-gb"));

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn report_rom_display_strips_the_family_prefix() {
        assert_eq!(
            report_rom_display("blargg", Path::new("blargg/cpu_instrs/01-special.gb")),
            "cpu_instrs/01-special.gb"
        );
    }

    #[test]
    fn report_rom_display_keeps_unrelated_paths_intact() {
        assert_eq!(
            report_rom_display("blargg", Path::new("other/case.gb")),
            "other/case.gb"
        );
    }

    #[test]
    fn curated_test_report_updates_merge_partial_suite_runs() {
        let workspace_root = unique_temp_dir("report-merge");
        fs::create_dir_all(gbemu_report_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let first_report = RomSuiteReport {
            suite_name: "blargg-cpu-instrs".to_string(),
            family: Some("blargg".to_string()),
            cases: vec![RomCaseReport {
                case_id: "blargg-cpu-instrs-01-special".to_string(),
                rom_path: PathBuf::from("blargg/cpu_instrs/01-special.gb"),
                outcome: RomCaseOutcome::Passed,
                executed_t_cycles: 0,
                completed_frames: 0,
                diagnostics: Vec::new(),
                artifacts: CapturedArtifacts::default(),
                retained_failure_artifacts: Vec::new(),
            }],
        };
        update_curated_test_report(&workspace_root, &first_report)
            .expect("first partial report should write");

        let second_report = RomSuiteReport {
            suite_name: "blargg-cpu-instrs".to_string(),
            family: Some("blargg".to_string()),
            cases: vec![RomCaseReport {
                case_id: "blargg-cpu-instrs-02-interrupts".to_string(),
                rom_path: PathBuf::from("blargg/cpu_instrs/02-interrupts.gb"),
                outcome: RomCaseOutcome::Failed(crate::RomCaseFailure::TimeoutExceeded),
                executed_t_cycles: 0,
                completed_frames: 0,
                diagnostics: Vec::new(),
                artifacts: CapturedArtifacts::default(),
                retained_failure_artifacts: Vec::new(),
            }],
        };
        let report_path = update_curated_test_report(&workspace_root, &second_report)
            .expect("second partial report should write")
            .expect("curated suite should emit a report path");

        let suite_status_path = gbemu_report_root(&workspace_root)
            .join(TEST_ROM_STATUS_DIR_NAME)
            .join("blargg-cpu-instrs.toml");
        let suite_status =
            fs::read_to_string(&suite_status_path).expect("suite status should be readable");
        assert!(suite_status.contains("cpu_instrs/01-special.gb"));
        assert!(suite_status.contains("cpu_instrs/02-interrupts.gb"));

        let rendered_report =
            fs::read_to_string(report_path).expect("markdown report should be readable");
        assert!(rendered_report.starts_with("# Test Report (1/2)\n"));
        assert!(rendered_report.contains(&format!(
            "| blargg | cpu_instrs/01-special.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(rendered_report.contains(&format!(
            "| blargg | cpu_instrs/02-interrupts.gb | {REPORT_STATUS_FAIL_EMOJI} |"
        )));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_updates_existing_case_status_and_ignores_non_toml_entries() {
        let workspace_root = unique_temp_dir("report-update");
        let status_root = gbemu_report_root(&workspace_root).join(TEST_ROM_STATUS_DIR_NAME);
        fs::create_dir_all(&status_root).expect("status root should be creatable");
        fs::write(status_root.join("README.txt"), "ignore me")
            .expect("non-toml status marker should be writable");

        let failing_report = RomSuiteReport {
            suite_name: "blargg-timing-memory-oam".to_string(),
            family: Some("blargg".to_string()),
            cases: vec![report_case(
                "blargg-halt-bug",
                "blargg/halt_bug.gb",
                RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded),
            )],
        };
        update_curated_test_report(&workspace_root, &failing_report)
            .expect("failing partial report should write");

        let passing_report = RomSuiteReport {
            suite_name: "blargg-timing-memory-oam".to_string(),
            family: Some("blargg".to_string()),
            cases: vec![report_case(
                "blargg-halt-bug",
                "blargg/halt_bug.gb",
                RomCaseOutcome::Passed,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &passing_report)
            .expect("passing partial report should write")
            .expect("curated suite should emit a report path");

        let suite_status_path = status_root.join("blargg-timing-memory-oam.toml");
        let suite_status =
            fs::read_to_string(&suite_status_path).expect("suite status should be readable");
        assert!(suite_status.contains("status = \"PASS\""));
        assert!(!suite_status.contains("status = \"FAIL\""));

        let rendered_report =
            fs::read_to_string(report_path).expect("markdown report should be readable");
        assert!(rendered_report.starts_with("# Test Report (1/1)\n"));
        assert!(rendered_report.contains(&format!(
            "| blargg | halt_bug.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(!rendered_report.contains(&format!(
            "| blargg | halt_bug.gb | {REPORT_STATUS_FAIL_EMOJI} |"
        )));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_persists_and_renders_informational_statuses() {
        let workspace_root = unique_temp_dir("report-info");
        fs::create_dir_all(gbemu_report_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let report = RomSuiteReport {
            suite_name: "acid".to_string(),
            family: Some("acid".to_string()),
            cases: vec![report_case(
                "acid-which-dmg",
                "acid/which.gb",
                RomCaseOutcome::Informational,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &report)
            .expect("informational report should write")
            .expect("curated suite should emit a report path");

        let suite_status_path = gbemu_report_root(&workspace_root)
            .join(TEST_ROM_STATUS_DIR_NAME)
            .join("acid.toml");
        let suite_status =
            fs::read_to_string(&suite_status_path).expect("suite status should be readable");
        assert!(suite_status.contains("status = \"INFO\""));

        let rendered_report =
            fs::read_to_string(report_path).expect("markdown report should be readable");
        assert!(rendered_report.starts_with("# Test Report (1/1)\n"));
        assert!(rendered_report.contains(&format!(
            "| acid | which.gb (DMG) | {REPORT_STATUS_INFO_EMOJI} |"
        )));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn family_reports_preserve_moved_cgb_smoke_suffixes() {
        let workspace_root = unique_temp_dir("report-family-cgb-rows");
        fs::create_dir_all(gbemu_report_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let acid_report = RomSuiteReport {
            suite_name: "acid".to_string(),
            family: Some("acid".to_string()),
            cases: vec![report_case(
                "acid-which-cgb",
                "acid/which.gb",
                RomCaseOutcome::Informational,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &acid_report)
            .expect("acid report should write")
            .expect("curated suite should emit a report path");
        let mooneye_report = RomSuiteReport {
            suite_name: "mooneye-acceptance-manual-misc".to_string(),
            family: Some("mooneye".to_string()),
            cases: vec![report_case(
                "mooneye-misc-boot-regs-cgb",
                "mooneye/misc/boot_regs-cgb.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &mooneye_report)
            .expect("Mooneye report should write");

        let acid_status_path = gbemu_report_root(&workspace_root)
            .join(TEST_ROM_STATUS_DIR_NAME)
            .join("acid.toml");
        let acid_status =
            fs::read_to_string(&acid_status_path).expect("acid status should be readable");
        assert!(acid_status.contains("rom = \"which.gb (GBC)\""));
        let mooneye_status_path = gbemu_report_root(&workspace_root)
            .join(TEST_ROM_STATUS_DIR_NAME)
            .join("mooneye-acceptance-manual-misc.toml");
        let mooneye_status =
            fs::read_to_string(&mooneye_status_path).expect("mooneye status should be readable");
        assert!(mooneye_status.contains("rom = \"misc/boot_regs-cgb.gb\""));

        let rendered_report =
            fs::read_to_string(report_path).expect("markdown report should be readable");
        assert!(rendered_report.starts_with("# Test Report (2/2)\n"));
        let acid_which = rendered_report
            .find(&format!(
                "| acid | which.gb (GBC) | {REPORT_STATUS_INFO_EMOJI} |"
            ))
            .expect("CGB Acid report row should exist");
        let mooneye_boot_regs = rendered_report
            .find(&format!(
                "| mooneye | misc/boot_regs-cgb.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("CGB Mooneye report row should exist");
        assert!(acid_which < mooneye_boot_regs);

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_prunes_disabled_manifest_rows_from_suite_status() {
        let workspace_root = unique_temp_dir("report-disabled-prune");
        let status_root = test_rom_store_root(&workspace_root).join(TEST_ROM_STATUS_DIR_NAME);
        fs::create_dir_all(&status_root).expect("status root should be creatable");
        fs::write(
            status_root.join("mooneye-cgb-extra.toml"),
            r#"suite_name = "mooneye-cgb-extra"
family = "mooneye"

[[cases]]
rom = "acceptance/ppu/intr_2_mode0_timing.gb"
status = "PASS"

[[cases]]
rom = "acceptance/ppu/lcdon_timing-GS.gb"
status = "FAIL"

[[cases]]
rom = "acceptance/ppu/vblank_stat_intr-GS.gb"
status = "FAIL"
"#,
        )
        .expect("stale Mooneye CGB status should be writable");

        let report = RomSuiteReport {
            suite_name: "mooneye-cgb-extra".to_string(),
            family: Some("mooneye".to_string()),
            cases: vec![report_case(
                "mooneye-cgb-ppu-intr-2-mode0-timing",
                "mooneye/acceptance/ppu/intr_2_mode0_timing.gb",
                RomCaseOutcome::Passed,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &report)
            .expect("Mooneye CGB extra report should write")
            .expect("extra curated suite should emit a report path");

        let suite_status = fs::read_to_string(status_root.join("mooneye-cgb-extra.toml"))
            .expect("Mooneye CGB status should be readable");
        assert!(suite_status.contains("intr_2_mode0_timing.gb"));
        assert!(!suite_status.contains("lcdon_timing-GS.gb"));
        assert!(!suite_status.contains("vblank_stat_intr-GS.gb"));

        let rendered_report =
            fs::read_to_string(report_path).expect("extra report should be readable");
        assert!(rendered_report.starts_with("# Test Report (1/1)\n"));
        assert!(rendered_report.contains(&format!(
            "| mooneye | acceptance/ppu/intr_2_mode0_timing.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(!rendered_report.contains("lcdon_timing-GS.gb"));
        assert!(!rendered_report.contains("vblank_stat_intr-GS.gb"));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_routes_extra_suites_to_extra_markdown_file() {
        let workspace_root = unique_temp_dir("report-cgb-extra");
        fs::create_dir_all(test_rom_store_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let mooneye_report = RomSuiteReport {
            suite_name: "mooneye-acceptance-manual-misc".to_string(),
            family: Some("mooneye".to_string()),
            cases: vec![report_case(
                "mooneye-misc-boot-regs-cgb",
                "mooneye/misc/boot_regs-cgb.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &mooneye_report)
            .expect("Mooneye report should write");

        let promoted_ax6_report = RomSuiteReport {
            suite_name: "ax6".to_string(),
            family: Some("ax6".to_string()),
            cases: vec![report_case(
                "ax6-rtc3test-1",
                "ax6/rtc3test-1.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &promoted_ax6_report)
            .expect("promoted AX6 report should write");

        let samesuite_apu_report = RomSuiteReport {
            suite_name: "samesuite-apu".to_string(),
            family: Some("samesuite".to_string()),
            cases: vec![
                report_case(
                    "samesuite-apu-div-write-trigger",
                    "samesuite/apu/div_write_trigger.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "samesuite-apu-div-write-trigger-10",
                    "samesuite/apu/div_write_trigger_10.gb",
                    RomCaseOutcome::Passed,
                ),
            ],
        };
        update_curated_test_report(&workspace_root, &samesuite_apu_report)
            .expect("CGB SameSuite report should write");

        let cgb_boot_hwio_report = RomSuiteReport {
            suite_name: "cgb-boot-hwio".to_string(),
            family: Some("cgb-boot-hwio".to_string()),
            cases: vec![report_case(
                "cgb-boot-hwio-boot-hwio-c",
                "mooneye/misc/boot_hwio-C.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &cgb_boot_hwio_report)
            .expect("CGB boot HWIO report should write")
            .expect("extra curated suite should emit a report path");

        let mooneye_cgb_report = RomSuiteReport {
            suite_name: "mooneye-cgb-extra".to_string(),
            family: Some("mooneye".to_string()),
            cases: vec![report_case(
                "mooneye-cgb-ppu-intr-2-mode0-timing",
                "mooneye/acceptance/ppu/intr_2_mode0_timing.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &mooneye_cgb_report)
            .expect("Mooneye CGB extra report should write")
            .expect("extra curated suite should emit a report path");

        let ax6_report = RomSuiteReport {
            suite_name: "ax6-dmg-extra".to_string(),
            family: Some("ax6".to_string()),
            cases: vec![
                report_case(
                    "ax6-dmg-rtc3test-1",
                    "ax6/rtc3test-1.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "ax6-dmg-rtc3test-2",
                    "ax6/rtc3test-2.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "ax6-dmg-rtc3test-3",
                    "ax6/rtc3test-3.gb",
                    RomCaseOutcome::Passed,
                ),
            ],
        };
        update_curated_test_report(&workspace_root, &ax6_report)
            .expect("AX6 DMG extra report should write")
            .expect("extra curated suite should emit a report path");

        let samesuite_report = RomSuiteReport {
            suite_name: "samesuite-dmg-extra".to_string(),
            family: Some("samesuite".to_string()),
            cases: vec![
                report_case(
                    "samesuite-dmg-div-write-trigger",
                    "samesuite/apu/div_write_trigger.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "samesuite-dmg-div-write-trigger-10",
                    "samesuite/apu/div_write_trigger_10.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "samesuite-dmg-ei-delay-halt",
                    "samesuite/interrupt/ei_delay_halt.gb",
                    RomCaseOutcome::Passed,
                ),
            ],
        };
        let _report_path = update_curated_test_report(&workspace_root, &samesuite_report)
            .expect("SameSuite DMG extra report should write")
            .expect("extra curated suite should emit a report path");

        let samesuite_cgb_report = RomSuiteReport {
            suite_name: "samesuite-cgb-extra".to_string(),
            family: Some("samesuite".to_string()),
            cases: vec![report_case(
                "samesuite-cgb-apu-channel-1-channel-1-align-12-cgbe",
                "samesuite/apu/channel_1/channel_1_align_12-cgbE.gb",
                RomCaseOutcome::Passed,
            )],
        };
        let _report_path = update_curated_test_report(&workspace_root, &samesuite_cgb_report)
            .expect("SameSuite CGB extra report should write")
            .expect("extra curated suite should emit a report path");

        let magen_report = RomSuiteReport {
            suite_name: "magen-cgb-extra".to_string(),
            family: Some("magen".to_string()),
            cases: vec![report_case(
                "magen-cgb-bg-oam-priority",
                "magen/bg_oam_priority.gbc",
                RomCaseOutcome::Passed,
            )],
        };
        let _report_path = update_curated_test_report(&workspace_root, &magen_report)
            .expect("Magen CGB extra report should write")
            .expect("extra curated suite should emit a report path");

        let little_things_report = RomSuiteReport {
            suite_name: "little-things-gb-dmg-extra".to_string(),
            family: Some("little-things-gb".to_string()),
            cases: vec![
                report_case(
                    "little-things-gb-dmg-double-halt-cancel",
                    "little-things-gb/double-halt-cancel.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "little-things-gb-dmg-whichboot",
                    "little-things-gb/whichboot.gb",
                    RomCaseOutcome::Passed,
                ),
            ],
        };
        let _report_path = update_curated_test_report(&workspace_root, &little_things_report)
            .expect("little-things-gb DMG extra report should write")
            .expect("extra curated suite should emit a report path");

        let little_things_cgb_report = RomSuiteReport {
            suite_name: "little-things-gb-cgb-extra".to_string(),
            family: Some("little-things-gb".to_string()),
            cases: vec![report_case(
                "little-things-gb-cgb-whichboot",
                "little-things-gb/whichboot.gb",
                RomCaseOutcome::Passed,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &little_things_cgb_report)
            .expect("little-things-gb CGB extra report should write")
            .expect("extra curated suite should emit a report path");
        assert_eq!(
            report_path,
            test_rom_store_root(&workspace_root).join(TEST_ROM_EXTRA_REPORT_FILE_NAME)
        );

        let standard_report =
            fs::read_to_string(gbemu_report_root(&workspace_root).join(TEST_ROM_REPORT_FILE_NAME))
                .expect("standard report should be readable");
        assert!(standard_report.starts_with("# Test Report (4/4)\n"));
        assert!(standard_report.contains(&format!(
            "| ax6 | rtc3test-1.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(standard_report.contains(&format!(
            "| mooneye | misc/boot_regs-cgb.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(standard_report.contains(&format!(
            "| samesuite | apu/div_write_trigger.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(standard_report.contains(&format!(
            "| samesuite | apu/div_write_trigger_10.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(!standard_report.contains("boot_hwio-C.gb"));
        assert!(!standard_report.contains("intr_2_mode0_timing.gb"));
        assert!(!standard_report.contains("div_write_trigger.gb (DMG)"));
        assert!(!standard_report.contains("div_write_trigger_10.gb (DMG)"));
        assert!(!standard_report.contains("ei_delay_halt.gb"));
        assert!(!standard_report.contains("rtc3test-1.gb (DMG)"));
        assert!(!standard_report.contains("whichboot.gb"));
        assert!(
            !test_rom_store_root(&workspace_root)
                .join(TEST_ROM_REPORT_FILE_NAME)
                .exists()
        );

        let extra_report =
            fs::read_to_string(report_path).expect("extra report should be readable");
        assert!(extra_report.starts_with("# Test Report (13/13)\n"));
        assert!(extra_report.contains(&format!(
            "| ax6 | rtc3test-1.gb (DMG) | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| ax6 | rtc3test-2.gb (DMG) | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| ax6 | rtc3test-3.gb (DMG) | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| mooneye | misc/boot_hwio-C.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| mooneye | acceptance/ppu/intr_2_mode0_timing.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| samesuite | apu/div_write_trigger.gb (DMG) | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| samesuite | apu/div_write_trigger_10.gb (DMG) | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| samesuite | interrupt/ei_delay_halt.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| samesuite | apu/channel_1/channel_1_align_12-cgbE.gb (GBC) | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| magen | bg_oam_priority.gbc | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| little-things-gb | double-halt-cancel.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| little-things-gb | whichboot.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(extra_report.contains(&format!(
            "| little-things-gb | whichboot.gb (GBC) | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(!extra_report.contains("boot_regs-cgb.gb"));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_omits_empty_standard_and_extra_markdown_files_for_docboy_only_runs() {
        let workspace_root = unique_temp_dir("report-docboy-only");
        let store_root = test_rom_store_root_for_report(&workspace_root, DOCBOY_REPORT_ID);
        fs::create_dir_all(&store_root).expect("DocBoy store root should be creatable");
        fs::write(
            store_root.join(TEST_ROM_REPORT_FILE_NAME),
            "# Test Report (0/0)\n",
        )
        .expect("stale standard report should be writable");
        fs::write(
            store_root.join(TEST_ROM_EXTRA_REPORT_FILE_NAME),
            "# Test Report (0/0)\n",
        )
        .expect("stale extra report should be writable");

        let docboy_report = RomSuiteReport {
            suite_name: "docboy-cgb".to_string(),
            family: Some("docboy-cgb".to_string()),
            cases: vec![report_case(
                "docboy-cgb-docboy-boot-boot-bg-palettes",
                "cgb/boot/boot_bg_palettes.gbc",
                RomCaseOutcome::Passed,
            )],
        };

        let report_path = update_curated_test_report(&workspace_root, &docboy_report)
            .expect("DocBoy report should write")
            .expect("DocBoy suite should emit a report path");

        assert_eq!(
            report_path,
            store_root.join(TEST_ROM_DOCBOY_REPORT_FILE_NAME)
        );
        assert!(report_path.exists());
        assert_eq!(report_path, store_root.join(TEST_ROM_REPORT_FILE_NAME));
        assert!(!store_root.join(TEST_ROM_EXTRA_REPORT_FILE_NAME).exists());
        assert!(
            !test_rom_store_root(&workspace_root)
                .join("test-report-docboy.md")
                .exists()
        );

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_routes_docboy_suites_to_docboy_markdown_file() {
        let workspace_root = unique_temp_dir("report-docboy");
        fs::create_dir_all(test_rom_store_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let mooneye_report = RomSuiteReport {
            suite_name: "mooneye-acceptance-manual-misc".to_string(),
            family: Some("mooneye".to_string()),
            cases: vec![report_case(
                "mooneye-misc-boot-regs-cgb",
                "mooneye/misc/boot_regs-cgb.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &mooneye_report)
            .expect("Mooneye report should write");

        let extra_report = RomSuiteReport {
            suite_name: "little-things-gb-dmg-extra".to_string(),
            family: Some("little-things-gb".to_string()),
            cases: vec![report_case(
                "little-things-gb-dmg-double-halt-cancel",
                "little-things-gb/double-halt-cancel.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &extra_report)
            .expect("extra report should write");

        let docboy_report = RomSuiteReport {
            suite_name: "docboy-cgb".to_string(),
            family: Some("docboy-cgb".to_string()),
            cases: vec![
                report_case(
                    "docboy-cgb-docboy-boot-boot-bg-palettes",
                    "cgb/boot/boot_bg_palettes.gbc",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "docboy-cgb-docboy-boot-boot-bg-palettes-fail",
                    "cgb/boot/boot_bg_palettes.gbc",
                    RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded),
                ),
            ],
        };
        let report_path = update_curated_test_report(&workspace_root, &docboy_report)
            .expect("DocBoy report should write")
            .expect("DocBoy suite should emit a report path");
        assert_eq!(
            report_path,
            test_rom_store_root_for_report(&workspace_root, DOCBOY_REPORT_ID)
                .join(TEST_ROM_DOCBOY_REPORT_FILE_NAME)
        );
        assert!(
            !test_rom_store_root(&workspace_root)
                .join("test-report-docboy.md")
                .exists()
        );

        let standard_report =
            fs::read_to_string(gbemu_report_root(&workspace_root).join(TEST_ROM_REPORT_FILE_NAME))
                .expect("standard report should be readable");
        assert!(standard_report.contains("boot_regs-cgb.gb"));
        assert!(!standard_report.contains("boot_bg_palettes.gbc"));
        assert!(!standard_report.contains("double-halt-cancel.gb"));

        let rendered_extra = fs::read_to_string(
            test_rom_store_root(&workspace_root).join(TEST_ROM_EXTRA_REPORT_FILE_NAME),
        )
        .expect("extra report should be readable");
        assert!(rendered_extra.contains("double-halt-cancel.gb"));
        assert!(!rendered_extra.contains("boot_bg_palettes.gbc"));

        let rendered_docboy =
            fs::read_to_string(report_path).expect("DocBoy report should be readable");
        assert!(rendered_docboy.starts_with("# Test Report (0/1)\n"));
        assert!(rendered_docboy.contains(&format!(
            "| docboy-cgb | boot/boot_bg_palettes.gbc | {REPORT_STATUS_FAIL_EMOJI} |"
        )));
        assert!(!rendered_docboy.contains("boot_regs-cgb.gb"));
        assert!(!rendered_docboy.contains("poweron_bgp_000.gb"));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_routes_gbmicrotest_suite_to_gbmicrotest_markdown_file() {
        let workspace_root = unique_temp_dir("report-gbmicrotest");
        let store_root = test_rom_store_root_for_report(&workspace_root, GBMICROTEST_REPORT_ID);
        fs::create_dir_all(&store_root).expect("gbmicrotest store root should be creatable");
        fs::write(
            store_root.join(TEST_ROM_EXTRA_REPORT_FILE_NAME),
            "# Test Report (0/0)\n",
        )
        .expect("stale extra report should be writable");

        let gbmicrotest_report = RomSuiteReport {
            suite_name: "gbmicrotest".to_string(),
            family: Some("gbmicrotest".to_string()),
            cases: vec![report_case(
                "gbmicrotest-halt-halt-bug",
                "test/gbmicrotest/halt/halt_bug.gb",
                RomCaseOutcome::Passed,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &gbmicrotest_report)
            .expect("gbmicrotest report should write")
            .expect("gbmicrotest suite should emit a report path");

        assert_eq!(
            report_path,
            store_root.join(TEST_ROM_GBMICROTEST_REPORT_FILE_NAME)
        );
        assert_eq!(
            fs::read_to_string(&report_path).expect("gbmicrotest report should be readable"),
            format!(
                "# Test Report (1/1)\n\n| family | rom | status |\n| --- | --- | --- |\n| gbmicrotest | halt/halt_bug.gb | {REPORT_STATUS_PASS_EMOJI} |\n"
            )
        );
        assert!(
            store_root
                .join(TEST_ROM_STATUS_DIR_NAME)
                .join("gbmicrotest.toml")
                .exists()
        );
        assert!(!store_root.join(TEST_ROM_EXTRA_REPORT_FILE_NAME).exists());
        assert!(
            !test_rom_store_root(&workspace_root)
                .join(TEST_ROM_EXTRA_REPORT_FILE_NAME)
                .exists()
        );

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_prunes_extra_model_rows_from_promoted_suite_status() {
        let workspace_root = unique_temp_dir("report-prune-extra-model-rows");
        let status_root = gbemu_report_root(&workspace_root).join(TEST_ROM_STATUS_DIR_NAME);
        fs::create_dir_all(&status_root).expect("status root should be creatable");
        fs::write(
            status_root.join("ax6.toml"),
            r#"suite_name = "ax6"
family = "ax6"

[[cases]]
family = "ax6"
rom = "rtc3test-1.gb"
status = "PASS"

[[cases]]
family = "ax6"
rom = "rtc3test-1.gb (DMG)"
status = "PASS"
"#,
        )
        .expect("stale promoted AX6 status should be writable");

        let promoted_ax6_report = RomSuiteReport {
            suite_name: "ax6".to_string(),
            family: Some("ax6".to_string()),
            cases: vec![report_case(
                "ax6-rtc3test-1",
                "ax6/rtc3test-1.gb",
                RomCaseOutcome::Passed,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &promoted_ax6_report)
            .expect("promoted AX6 report should write")
            .expect("curated suite should emit a report path");

        let suite_status = fs::read_to_string(status_root.join("ax6.toml"))
            .expect("promoted AX6 status should be readable");
        assert!(suite_status.contains("rom = \"rtc3test-1.gb\""));
        assert!(!suite_status.contains("rtc3test-1.gb (DMG)"));

        let standard_report =
            fs::read_to_string(report_path).expect("markdown report should be readable");
        assert!(standard_report.starts_with("# Test Report (1/1)\n"));
        assert!(standard_report.contains(&format!(
            "| ax6 | rtc3test-1.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(!standard_report.contains("rtc3test-1.gb (DMG)"));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn render_markdown_report_does_not_infer_family_from_mismatched_full_path_rows() {
        let rendered = render_markdown_report(&[PersistedSuiteStatus {
            suite_name: "future-mixed".to_string(),
            family: "future-mixed".to_string(),
            cases: vec![PersistedCaseStatus {
                family: None,
                rom: "mooneye/misc/boot_regs-cgb.gb".to_string(),
                status: "PASS".to_string(),
            }],
        }]);

        assert!(rendered.starts_with("# Test Report (1/1)\n"));
        assert!(rendered.contains(&format!(
            "| future-mixed | mooneye/misc/boot_regs-cgb.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(!rendered.contains(&format!(
            "| mooneye | misc/boot_regs-cgb.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
    }

    #[test]
    fn render_markdown_report_orders_mixed_rows_by_shootout_source_order() {
        let rendered = render_markdown_report(&[
            PersistedSuiteStatus {
                suite_name: "acid".to_string(),
                family: "acid".to_string(),
                cases: vec![
                    PersistedCaseStatus {
                        family: None,
                        rom: "dmg-acid2.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                    PersistedCaseStatus {
                        family: None,
                        rom: "which.gb (DMG)".to_string(),
                        status: "INFO".to_string(),
                    },
                    PersistedCaseStatus {
                        family: None,
                        rom: "which.gb (GBC)".to_string(),
                        status: "INFO".to_string(),
                    },
                ],
            },
            PersistedSuiteStatus {
                suite_name: "mooneye-acceptance-manual-misc".to_string(),
                family: "mooneye".to_string(),
                cases: vec![
                    PersistedCaseStatus {
                        family: None,
                        rom: "manual-only/sprite_priority.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                    PersistedCaseStatus {
                        family: None,
                        rom: "misc/boot_regs-cgb.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                ],
            },
        ]);

        let acid_which_dmg = rendered
            .find(&format!(
                "| acid | which.gb (DMG) | {REPORT_STATUS_INFO_EMOJI} |"
            ))
            .expect("Acid DMG which row should exist");
        let acid_which_gbc = rendered
            .find(&format!(
                "| acid | which.gb (GBC) | {REPORT_STATUS_INFO_EMOJI} |"
            ))
            .expect("Acid GBC which row should exist");
        let acid_dmg_acid2 = rendered
            .find(&format!(
                "| acid | dmg-acid2.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("Acid acid-dmg-acid2 row should exist");
        let mooneye_sprite_priority = rendered
            .find(&format!(
                "| mooneye | manual-only/sprite_priority.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("Mooneye sprite_priority row should exist");
        let mooneye_boot_regs_cgb = rendered
            .find(&format!(
                "| mooneye | misc/boot_regs-cgb.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("Mooneye CGB boot_regs row should exist");

        assert!(acid_which_dmg < acid_which_gbc);
        assert!(acid_which_gbc < acid_dmg_acid2);
        assert!(mooneye_sprite_priority < mooneye_boot_regs_cgb);
    }

    #[test]
    fn render_markdown_report_orders_samesuite_rows_after_mooneye_rows() {
        let rendered = render_markdown_report(&[
            PersistedSuiteStatus {
                suite_name: "mooneye-acceptance-manual-misc".to_string(),
                family: "mooneye".to_string(),
                cases: vec![PersistedCaseStatus {
                    family: None,
                    rom: "misc/boot_regs-cgb.gb".to_string(),
                    status: "PASS".to_string(),
                }],
            },
            PersistedSuiteStatus {
                suite_name: "samesuite".to_string(),
                family: "samesuite".to_string(),
                cases: vec![
                    PersistedCaseStatus {
                        family: None,
                        rom: "sgb/command_mlt_req.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                    PersistedCaseStatus {
                        family: None,
                        rom: "sgb/command_mlt_req_1_incrementing.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                    PersistedCaseStatus {
                        family: None,
                        rom: "ppu/blocking_bgpi_increase.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                ],
            },
        ]);

        let mooneye_boot_regs = rendered
            .find(&format!(
                "| mooneye | misc/boot_regs-cgb.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("Mooneye CGB boot_regs row should exist");
        let blocking = rendered
            .find(&format!(
                "| samesuite | ppu/blocking_bgpi_increase.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("SameSuite CGB blocking BGPI row should exist");
        let mlt_req = rendered
            .find(&format!(
                "| samesuite | sgb/command_mlt_req.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("SameSuite SGB MLT_REQ row should exist");
        let mlt_req_incrementing = rendered
            .find(&format!(
                "| samesuite | sgb/command_mlt_req_1_incrementing.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("SameSuite SGB MLT_REQ incrementing row should exist");

        assert!(mooneye_boot_regs < blocking);
        assert!(blocking < mlt_req);
        assert!(mlt_req < mlt_req_incrementing);
    }

    #[test]
    fn curated_test_report_returns_none_for_non_curated_suites() {
        let workspace_root = unique_temp_dir("report-none");
        let report = RomSuiteReport {
            suite_name: "phase-2-cpu-timing".to_string(),
            family: None,
            cases: vec![report_case(
                "phase-2-fetch-immediate-order",
                "phase2/fetch_immediate_order.gb",
                RomCaseOutcome::Passed,
            )],
        };

        assert_eq!(
            update_curated_test_report(&workspace_root, &report)
                .expect("non-curated suite should not fail"),
            None
        );
        assert!(!test_rom_store_root(&workspace_root).exists());
    }

    #[test]
    fn curated_test_report_error_paths_report_target_context() {
        let report = RomSuiteReport {
            suite_name: "acid".to_string(),
            family: Some("acid".to_string()),
            cases: vec![report_case(
                "acid-which-dmg",
                "acid/which.gb",
                RomCaseOutcome::Informational,
            )],
        };

        let workspace_file = unique_temp_dir("report-workspace-file");
        fs::write(&workspace_file, "not-a-directory").expect("workspace file should be writable");
        let error = update_curated_test_report(&workspace_file, &report)
            .expect_err("workspace file should reject report store creation");
        assert!(error.contains("failed to create curated test ROM store"));
        fs::remove_file(&workspace_file).expect("workspace file should be removable");

        let workspace_root = unique_temp_dir("report-status-file");
        let status_root = gbemu_report_root(&workspace_root).join(TEST_ROM_STATUS_DIR_NAME);
        fs::create_dir_all(
            status_root
                .parent()
                .expect("status root should have a parent"),
        )
        .expect("store root should be creatable");
        fs::write(&status_root, "not-a-directory").expect("status file should be writable");
        let error = update_curated_test_report(&workspace_root, &report)
            .expect_err("status file should reject status root creation");
        assert!(error.contains("failed to create curated test ROM status root"));
        fs::remove_dir_all(&workspace_root).expect("workspace root should be removable");

        let workspace_root = unique_temp_dir("report-output-dir");
        let report_path = gbemu_report_root(&workspace_root).join(TEST_ROM_REPORT_FILE_NAME);
        fs::create_dir_all(&report_path).expect("report path directory should be creatable");
        let error = update_curated_test_report(&workspace_root, &report)
            .expect_err("report directory should reject markdown write");
        assert!(error.contains("failed to write curated test ROM report"));
        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn render_markdown_report_orders_present_families_without_placeholders() {
        let rendered = render_markdown_report(&[
            PersistedSuiteStatus {
                suite_name: "mooneye-acceptance-manual-misc".to_string(),
                family: "mooneye".to_string(),
                cases: vec![
                    PersistedCaseStatus {
                        family: None,
                        rom: "acceptance/div_timing.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                    PersistedCaseStatus {
                        family: None,
                        rom: "acceptance/add_sp_e_timing.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                ],
            },
            PersistedSuiteStatus {
                suite_name: "acid".to_string(),
                family: "acid".to_string(),
                cases: vec![
                    PersistedCaseStatus {
                        family: None,
                        rom: "which.gb".to_string(),
                        status: "INFO".to_string(),
                    },
                    PersistedCaseStatus {
                        family: None,
                        rom: "dmg-acid2.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                ],
            },
        ]);

        assert!(rendered.starts_with("# Test Report (4/4)\n"));
        let acid_which = rendered
            .find(&format!(
                "| acid | which.gb (DMG) | {REPORT_STATUS_INFO_EMOJI} |"
            ))
            .expect("acid informational row should exist");
        let acid_dmg = rendered
            .find(&format!(
                "| acid | dmg-acid2.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("acid framebuffer row should exist");
        let mooneye_div = rendered
            .find(&format!(
                "| mooneye | acceptance/div_timing.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("first mooneye row should exist");
        let mooneye_add = rendered
            .find(&format!(
                "| mooneye | acceptance/add_sp_e_timing.gb | {REPORT_STATUS_PASS_EMOJI} |"
            ))
            .expect("second mooneye row should exist");
        assert!(acid_which < acid_dmg);
        assert!(acid_dmg < mooneye_div);
        assert!(mooneye_add < mooneye_div);
        assert!(!rendered.contains("| blargg | - | - |"));
        assert!(!rendered.contains("| daid | - | - |"));
        assert!(!rendered.contains("| ax6 | - | - |"));
        assert!(!rendered.contains("| samesuite | - | - |"));
        assert!(!rendered.contains("| ashiepaws | - | - |"));
        assert!(!rendered.contains("| cpp | - | - |"));
        assert!(!rendered.contains("| mealybug-tearoom-tests | - | - |"));
        assert!(!rendered.contains("| little-things-gb | - | - |"));
    }

    #[test]
    fn render_markdown_report_keeps_unknown_families_when_they_are_present() {
        let rendered = render_markdown_report(&[PersistedSuiteStatus {
            suite_name: "future-dmg-curated".to_string(),
            family: "future".to_string(),
            cases: vec![PersistedCaseStatus {
                family: None,
                rom: "probe.gb".to_string(),
                status: "INFO".to_string(),
            }],
        }]);

        assert!(rendered.starts_with("# Test Report (1/1)\n"));
        let future_case = rendered
            .find(&format!(
                "| future | probe.gb | {REPORT_STATUS_INFO_EMOJI} |"
            ))
            .expect("unknown family row should exist");
        assert!(future_case > 0);
        assert!(!rendered.contains("| acid | - | - |"));
        assert!(!rendered.contains("| mealybug-tearoom-tests | - | - |"));
        assert!(!rendered.contains("| little-things-gb | - | - |"));
    }

    #[test]
    fn curated_test_report_header_counts_all_persisted_context_after_partial_family_update() {
        let workspace_root = unique_temp_dir("report-summary-context");
        fs::create_dir_all(test_rom_store_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let acid_report = RomSuiteReport {
            suite_name: "acid".to_string(),
            family: Some("acid".to_string()),
            cases: vec![
                report_case(
                    "acid-which-dmg",
                    "acid/which.gb",
                    RomCaseOutcome::Informational,
                ),
                report_case(
                    "acid-dmg-acid2",
                    "acid/dmg-acid2.gb",
                    RomCaseOutcome::Passed,
                ),
            ],
        };
        update_curated_test_report(&workspace_root, &acid_report)
            .expect("acid report should write");

        let blargg_report = RomSuiteReport {
            suite_name: "blargg-timing-memory-oam".to_string(),
            family: Some("blargg".to_string()),
            cases: vec![report_case(
                "blargg-halt-bug",
                "blargg/halt_bug.gb",
                RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded),
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &blargg_report)
            .expect("blargg partial report should write")
            .expect("curated suite should emit a report path");

        let rendered_report =
            fs::read_to_string(report_path).expect("markdown report should be readable");
        assert!(rendered_report.starts_with("# Test Report (2/3)\n"));
        assert!(rendered_report.contains(&format!(
            "| acid | which.gb (DMG) | {REPORT_STATUS_INFO_EMOJI} |"
        )));
        assert!(rendered_report.contains(&format!(
            "| acid | dmg-acid2.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(rendered_report.contains(&format!(
            "| blargg | halt_bug.gb | {REPORT_STATUS_FAIL_EMOJI} |"
        )));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn sort_persisted_case_statuses_uses_manifest_order_before_lexical_fallback() {
        let mut case_statuses = vec![
            PersistedCaseStatus {
                family: None,
                rom: "dmg-acid2.gb".to_string(),
                status: "PASS".to_string(),
            },
            PersistedCaseStatus {
                family: None,
                rom: "which.gb".to_string(),
                status: "INFO".to_string(),
            },
            PersistedCaseStatus {
                family: None,
                rom: "zzz-untracked.gb".to_string(),
                status: "INFO".to_string(),
            },
        ];

        sort_persisted_case_statuses("acid", "acid", &mut case_statuses);

        assert_eq!(case_statuses[0].rom, "which.gb");
        assert_eq!(case_statuses[1].rom, "dmg-acid2.gb");
        assert_eq!(case_statuses[2].rom, "zzz-untracked.gb");
    }

    #[test]
    fn report_status_display_maps_persisted_statuses_to_emojis() {
        assert_eq!(report_status_display("PASS"), REPORT_STATUS_PASS_EMOJI);
        assert_eq!(report_status_display("FAIL"), REPORT_STATUS_FAIL_EMOJI);
        assert_eq!(report_status_display("INFO"), REPORT_STATUS_INFO_EMOJI);
    }

    #[test]
    #[should_panic(expected = "unsupported curated test ROM report status")]
    fn report_status_display_rejects_unknown_statuses() {
        let _ = report_status_display("BROKEN");
    }

    #[test]
    fn load_persisted_suite_status_rejects_invalid_toml() {
        let workspace_root = unique_temp_dir("invalid-status");
        let path = workspace_root.join("invalid.toml");
        fs::create_dir_all(&workspace_root).expect("workspace root should be creatable");
        fs::write(&path, "suite_name = [").expect("invalid status file should be writable");

        let error = load_persisted_suite_status(&path)
            .expect_err("invalid persisted status should be rejected");
        assert!(error.contains("failed to parse curated test ROM status"));

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn trace_fixture_policies_fall_back_to_debugging_minimum() {
        let pass_condition = PassCondition::TraceFixture(PathBuf::from("fixture.trace"));

        let capture_plan = capture_plan_for_pass_condition(&pass_condition);
        assert!(capture_plan.contains(CaptureKind::Trace));
        assert!(capture_plan.contains(CaptureKind::Snapshot));

        let failure_artifacts = failure_artifacts_for_pass_condition(&pass_condition);
        assert!(failure_artifacts.contains(CaptureKind::Trace));
        assert!(failure_artifacts.contains(CaptureKind::Snapshot));
    }

    #[test]
    #[should_panic(expected = "unsupported console model")]
    fn parse_manifest_console_model_rejects_unknown_values() {
        let _ = parse_manifest_console_model("test-manifest.toml", "test-case", "super-game-boy");
    }

    #[test]
    fn parse_manifest_console_profile_supports_sgb_hosts() {
        assert_eq!(
            parse_manifest_console_model("test-manifest.toml", "test-case", "sgb"),
            ConsoleModel::GameBoy
        );
        assert_eq!(
            parse_manifest_host_platform("test-manifest.toml", "test-case", "sgb"),
            HostPlatform::Sgb
        );
        assert_eq!(
            parse_manifest_console_model("test-manifest.toml", "test-case", "sgb2"),
            ConsoleModel::GameBoy
        );
        assert_eq!(
            parse_manifest_host_platform("test-manifest.toml", "test-case", "sgb2"),
            HostPlatform::Sgb2
        );
    }

    #[test]
    fn parse_manifest_applies_case_defaults_from_manifest_header() {
        let manifest = parse_manifest(
            "test-manifest.toml",
            r#"
family = "defaulted"
suite_name = "ax6-dmg-extra"
source_id = "custom-source"
console = "cgb"
startup = "custom-boot"
execution_mode = "permissive"
report_console_suffix = true
timeout_frames = 180
oracle = "info-framebuffer"
memory = [{ address = 65410, value = 1 }]

[[stimulus]]
tcycle = 4
button = "a"
pressed = true

[[case]]
id = "inherited"
rom = "inherited.gb"

[[case]]
id = "overridden"
rom = "overridden.gb"
console = "dmg"
timeout_frames = 30
oracle = "serial-contains"
expected = "Passed"
memory = []
stimulus = []
"#,
        );

        assert_eq!(manifest.cases.len(), 2);
        assert_eq!(manifest.cases[0].source_id, "custom-source");
        assert_eq!(
            manifest.cases[0].source_path,
            Path::new("testroms/defaulted/inherited.gb")
        );
        assert_eq!(manifest.cases[0].console_model, ConsoleModel::GameBoyColor);
        assert_eq!(manifest.cases[0].startup_mode, StartupMode::CustomBoot);
        assert_eq!(
            manifest.cases[0].execution_mode.as_deref(),
            Some("permissive")
        );
        assert!(manifest.cases[0].report_console_suffix);
        assert_eq!(manifest.cases[0].timeout, Timeout::Frames(180));
        assert_eq!(manifest.cases[0].oracle, "info-framebuffer");
        assert_eq!(manifest.cases[0].memory.len(), 1);
        assert_eq!(manifest.cases[0].stimuli.len(), 1);

        assert_eq!(manifest.cases[1].console_model, ConsoleModel::GameBoy);
        assert_eq!(manifest.cases[1].timeout, Timeout::Frames(30));
        assert_eq!(manifest.cases[1].oracle, "serial-contains");
        assert_eq!(manifest.cases[1].expected.as_deref(), Some("Passed"));
        assert!(manifest.cases[1].report_console_suffix);
        assert!(manifest.cases[1].memory.is_empty());
        assert!(manifest.cases[1].stimuli.is_empty());
    }

    #[test]
    #[should_panic(expected = "missing family")]
    fn parse_manifest_case_rejects_familyless_mixed_cases() {
        let _ = parse_manifest_case(
            "test-manifest.toml",
            None,
            None,
            &CuratedTestRomCaseDefaultsFile::default(),
            CuratedTestRomCaseFile {
                family: None,
                id: "familyless".to_string(),
                rom: PathBuf::from("familyless.gb"),
                source_id: None,
                source_path: None,
                report_console_suffix: None,
                report_label: None,
                timeout_frames: Some(1),
                timeout_tcycles: None,
                oracle: Some("info-framebuffer".to_string()),
                expected: None,
                fixture: None,
                check_interval_tcycles: None,
                check_at_tcycles: None,
                memory: None,
                stimuli: None,
                console: None,
                revision: None,
                startup: None,
                execution_mode: None,
                stop_condition: None,
                disabled: None,
                comment: None,
            },
        );
    }

    #[test]
    fn parse_manifest_case_preserves_disabled_case_comment() {
        let case = parse_manifest_case(
            "test-manifest.toml",
            Some("docboy-dmg"),
            None,
            &CuratedTestRomCaseDefaultsFile::default(),
            CuratedTestRomCaseFile {
                family: None,
                id: "disabled-with-comment".to_string(),
                rom: PathBuf::from("disabled.gb"),
                source_id: None,
                source_path: None,
                report_console_suffix: None,
                report_label: None,
                timeout_frames: Some(1),
                timeout_tcycles: None,
                oracle: Some("info-framebuffer".to_string()),
                expected: None,
                fixture: None,
                check_interval_tcycles: None,
                check_at_tcycles: None,
                memory: None,
                stimuli: None,
                console: Some("dmg".to_string()),
                revision: None,
                startup: None,
                execution_mode: None,
                stop_condition: None,
                disabled: Some(true),
                comment: Some("  hardware-incompatible oracle  ".to_string()),
            },
        );

        assert!(case.disabled);
        assert_eq!(
            case.comment.as_deref(),
            Some("hardware-incompatible oracle")
        );
    }

    #[test]
    #[should_panic(expected = "disabled curated case")]
    fn parse_manifest_case_rejects_disabled_case_without_comment() {
        let _ = parse_manifest_case(
            "test-manifest.toml",
            Some("docboy-dmg"),
            None,
            &CuratedTestRomCaseDefaultsFile::default(),
            CuratedTestRomCaseFile {
                family: None,
                id: "disabled-without-comment".to_string(),
                rom: PathBuf::from("disabled.gb"),
                source_id: None,
                source_path: None,
                report_console_suffix: None,
                report_label: None,
                timeout_frames: Some(1),
                timeout_tcycles: None,
                oracle: Some("info-framebuffer".to_string()),
                expected: None,
                fixture: None,
                check_interval_tcycles: None,
                check_at_tcycles: None,
                memory: None,
                stimuli: None,
                console: Some("dmg".to_string()),
                revision: None,
                startup: None,
                execution_mode: None,
                stop_condition: None,
                disabled: Some(true),
                comment: Some("   ".to_string()),
            },
        );
    }

    #[test]
    #[should_panic(expected = "unsupported oracle")]
    fn manifest_case_to_rom_test_case_rejects_unknown_oracles() {
        let _ = manifest_case_to_rom_test_case(
            CuratedTestRomCase {
                family: "blargg".to_string(),
                id: "bad-oracle".to_string(),
                rom: PathBuf::from("bad.gb"),
                source_id: GBEMU_SHOOTOUT_SOURCE_ID.to_string(),
                source_path: PathBuf::from("testroms/blargg/bad.gb"),
                report_console_suffix: false,
                report_label: None,
                timeout: Timeout::Frames(1),
                oracle: "unknown".to_string(),
                expected: None,
                fixture: None,
                check_interval_tcycles: None,
                check_at_tcycles: None,
                memory: Vec::new(),
                stimuli: Vec::new(),
                console_model: ConsoleModel::GameBoy,
                host_platform: HostPlatform::Handheld,
                revision: HardwareRevision::DmgCpuC,
                startup_mode: StartupMode::SkipBoot,
                execution_mode: None,
                stop_condition: None,
                disabled: false,
                comment: None,
            },
            None,
        );
    }

    #[test]
    fn report_file_name_stays_stable() {
        assert_eq!(TEST_ROM_REPORT_FILE_NAME, "test-report.md");
        assert_eq!(TEST_ROM_EXTRA_REPORT_FILE_NAME, "test-report-extra.md");
        assert_eq!(TEST_ROM_DOCBOY_REPORT_FILE_NAME, "test-report.md");
        assert_eq!(TEST_ROM_DOCBOY_REPORT_DIR, DOCBOY_REPORT_ID);
        assert_eq!(TEST_ROM_GBMICROTEST_REPORT_FILE_NAME, "test-report.md");
        assert_eq!(TEST_ROM_GBMICROTEST_REPORT_DIR, GBMICROTEST_REPORT_ID);
        assert!(suite_uses_extra_test_report("ax6-dmg-extra"));
        assert!(suite_uses_extra_test_report("cgb-boot-hwio"));
        assert!(suite_uses_extra_test_report("mooneye-cgb-extra"));
        assert!(suite_uses_extra_test_report("samesuite-dmg-extra"));
        assert!(suite_uses_extra_test_report("mealybug-tearoom-cgb-extra"));
        assert!(suite_uses_extra_test_report("little-things-gb-dmg-extra"));
        assert!(!suite_uses_extra_test_report("gbmicrotest"));
        assert!(suite_uses_gbmicrotest_test_report("gbmicrotest"));
        assert!(suite_uses_docboy_test_report("docboy-dmg"));
        assert!(suite_uses_docboy_test_report("docboy-cgb"));
        assert!(suite_uses_docboy_test_report("docboy-cgb-dmg"));
        assert!(suite_uses_docboy_test_report("docboy-cgb-dmg-ext"));
        assert!(!suite_uses_extra_test_report("docboy-dmg"));
        assert!(!suite_uses_extra_test_report("docboy-cgb"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-dmg"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-dmg-ext"));
    }

    #[test]
    fn standard_report_family_order_matches_promoted_markdown_report_families() {
        assert_eq!(
            CURATED_TEST_ROM_REPORT_FAMILY_ORDER,
            [
                "acid",
                "blargg",
                "daid",
                "ax6",
                "mooneye",
                "samesuite",
                "ashiepaws",
                "cpp",
                "mealybug-tearoom-tests",
            ]
        );

        for family in [
            "magen",
            "gbmicrotest",
            "docboy-dmg",
            "docboy-cgb",
            "docboy-cgb-dmg",
            "docboy-cgb-dmg-ext",
            "little-things-gb",
        ] {
            assert_eq!(
                report_family_rank(
                    family,
                    report_family_order_for_kind(CuratedTestReportKind::Standard)
                ),
                None,
                "{family} should not be ranked in {TEST_ROM_REPORT_FILE_NAME}"
            );
        }
        assert!(
            report_family_rank(
                "magen",
                report_family_order_for_kind(CuratedTestReportKind::Extra)
            )
            .is_some()
        );
        assert_eq!(
            report_family_rank(
                "gbmicrotest",
                report_family_order_for_kind(CuratedTestReportKind::Extra)
            ),
            None
        );
        assert!(
            report_family_rank(
                "gbmicrotest",
                report_family_order_for_kind(CuratedTestReportKind::Gbmicrotest)
            )
            .is_some()
        );
        assert!(
            report_family_rank(
                "docboy-dmg",
                report_family_order_for_kind(CuratedTestReportKind::DocBoy)
            )
            .is_some()
        );
    }
}
