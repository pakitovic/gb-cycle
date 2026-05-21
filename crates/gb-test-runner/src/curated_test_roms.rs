use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gb_core::{ConsoleModel, HardwareRevision, JoypadButton, StartupMode};
use serde::{Deserialize, Serialize};

use crate::{
    CaptureKind, CapturePlan, ExecutionMode, ExecutionStopCondition, ExternalStimulus,
    ExternalStimulusAction, FailureArtifactPolicy, MemoryByteExpectation, MemoryTextOutputSpec,
    PassCondition, RomSuite, RomSuiteReport, RomTestCase, TestSubsystem, Timeout,
};

pub const TEST_ROM_STORE_DIR: &str = ".roms/test";
pub const TEST_ROM_ROOT_ENV_VAR: &str = "GB_CYCLE_TEST_ROM_ROOT";
pub const TEST_ROM_REPORT_FILE_NAME: &str = "test-report.md";
pub const TEST_ROM_EXTRA_REPORT_FILE_NAME: &str = "test-report-extra.md";
pub const TEST_ROM_DOCBOY_REPORT_FILE_NAME: &str = "test-report-docboy.md";

const TEST_ROM_STATUS_DIR_NAME: &str = ".status";
const CURATED_TEST_ROM_MANIFEST_VERSION: u32 = 1;
const CURATED_SOURCE_MANIFEST_VERSION: u32 = 1;
const CURATED_TEST_ROM_REPORT_VERSION: u32 = 1;
const GBEMU_SHOOTOUT_SOURCE_ID: &str = "gbemu-shootout";
const GBEMU_SHOOTOUT_TESTROMS_DIR: &str = "testroms";
const REPORT_STATUS_PASS_EMOJI: &str = "✅";
const REPORT_STATUS_FAIL_EMOJI: &str = "❌";
const REPORT_STATUS_INFO_EMOJI: &str = "ℹ️";
static CURATED_TEST_ROM_MANIFEST_CACHE: OnceLock<Vec<CuratedTestRomManifest>> = OnceLock::new();
static CURATED_SOURCE_ROM_PATH_CACHE: OnceLock<Vec<(String, PathBuf)>> = OnceLock::new();
static CURATED_SOURCE_ROM_ORDER_CACHE: OnceLock<BTreeMap<(String, PathBuf), usize>> =
    OnceLock::new();
const CURATED_TEST_ROM_REPORT_FAMILY_ORDER: [&str; 16] = [
    "acid",
    "blargg",
    "daid",
    "ax6",
    "mooneye",
    "samesuite",
    "magen",
    "gbmicrotest",
    "docboy-dmg",
    "docboy-cgb",
    "docboy-cgb-dmg",
    "docboy-cgb-dmg-ext",
    "hacktix",
    "cpp",
    "mealybug-tearoom-tests",
    "little-things-gb",
];
const EXTRA_CURATED_TEST_ROM_REPORT_SUITE_NAMES: [&str; 8] = [
    "ax6-dmg-extra",
    "cgb-boot-hwio",
    "samesuite-dmg-extra",
    "samesuite-cgb-extra",
    "magen-cgb-extra",
    "gbmicrotest-dmg-extra",
    "little-things-gb-dmg-extra",
    "little-things-gb-cgb-extra",
];
const DOCBOY_CURATED_TEST_ROM_REPORT_SUITE_NAMES: [&str; 4] = [
    "docboy-dmg-extra",
    "docboy-cgb-extra",
    "docboy-cgb-dmg-extra",
    "docboy-cgb-dmg-ext-extra",
];
const STATUS_ONLY_CURATED_TEST_ROM_REPORT_SUITE_NAMES: [&str; 1] =
    ["mooneye-acceptance-dmg-curated"];
/// Mealybug DMG rows where GBEmulatorShootout marks SameBoy as non-PASS in the 2026-03-22 table.
pub const MEALYBUG_SAMEBOY_SHOOTOUT_NON_PASS_CASE_IDS: &[&str] = &[
    "mealybug-m3-lcdc-bg-en-change",
    "mealybug-m3-lcdc-bg-map-change",
    "mealybug-m3-lcdc-obj-size-change",
    "mealybug-m3-lcdc-obj-size-change-scx",
    "mealybug-m3-lcdc-tile-sel-change",
    "mealybug-m3-lcdc-tile-sel-win-change",
    "mealybug-m3-lcdc-win-en-change-multiple-wx",
    "mealybug-m3-lcdc-win-map-change",
    "mealybug-m3-scy-change",
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedTestRomManifestFile {
    version: u32,
    family: Option<String>,
    suite_name: String,
    subsystem: String,
    #[serde(rename = "case")]
    cases: Vec<CuratedTestRomCaseFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedTestRomCaseFile {
    family: Option<String>,
    id: String,
    rom: PathBuf,
    source_id: Option<String>,
    source_path: Option<PathBuf>,
    report_model_suffix: Option<bool>,
    report_label: Option<String>,
    timeout_frames: Option<u32>,
    timeout_tcycles: Option<u64>,
    oracle: String,
    expected: Option<String>,
    fixture: Option<PathBuf>,
    fixtures: Option<Vec<PathBuf>>,
    check_interval_tcycles: Option<u64>,
    check_at_tcycles: Option<u64>,
    #[serde(default)]
    memory: Vec<CuratedMemoryByteExpectationFile>,
    #[serde(rename = "stimulus", default)]
    stimuli: Vec<CuratedRomStimulusFile>,
    console: Option<String>,
    revision: Option<String>,
    startup: Option<String>,
    execution_mode: Option<String>,
    stop_condition: Option<String>,
    #[serde(default)]
    disabled: bool,
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
    subsystem: TestSubsystem,
    cases: Vec<CuratedTestRomCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CuratedTestRomCase {
    family: String,
    id: String,
    rom: PathBuf,
    source_id: String,
    source_path: PathBuf,
    report_model_suffix: bool,
    report_label: Option<String>,
    timeout: Timeout,
    oracle: String,
    expected: Option<String>,
    fixture: Option<PathBuf>,
    fixtures: Option<Vec<PathBuf>>,
    check_interval_tcycles: Option<u64>,
    check_at_tcycles: Option<u64>,
    memory: Vec<MemoryByteExpectation>,
    stimuli: Vec<ExternalStimulus>,
    console_model: ConsoleModel,
    revision: HardwareRevision,
    startup_mode: StartupMode,
    execution_mode: Option<String>,
    stop_condition: Option<String>,
    disabled: bool,
    comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedSuiteStatus {
    version: u32,
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
    version: u32,
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
}

pub fn test_rom_store_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(TEST_ROM_STORE_DIR)
}

pub fn discover_test_rom_store_root(workspace_root: &Path) -> Option<PathBuf> {
    if let Some(root) = env::var_os(TEST_ROM_ROOT_ENV_VAR) {
        return Some(PathBuf::from(root));
    }

    let default_root = test_rom_store_root(workspace_root);
    default_root.exists().then_some(default_root)
}

pub(crate) fn curated_family_store_prefix(family: &str) -> PathBuf {
    match family {
        "docboy-dmg" => PathBuf::from("docboy").join("dmg"),
        "docboy-cgb" => PathBuf::from("docboy").join("cgb"),
        "docboy-cgb-dmg" => PathBuf::from("docboy").join("cgb-dmg"),
        "docboy-cgb-dmg-ext" => PathBuf::from("docboy").join("cgb-dmg-ext"),
        _ => PathBuf::from(family),
    }
}

pub(crate) fn curated_case_store_relative_path(family: &str, rom: &Path) -> PathBuf {
    curated_family_store_prefix(family).join(rom)
}

pub fn acid_dmg_curated_suite() -> RomSuite {
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

pub fn magen_cgb_extra_suite() -> RomSuite {
    manifest_suite_by_name("magen-cgb-extra")
}

pub fn little_things_gb_dmg_extra_suite() -> RomSuite {
    manifest_suite_by_name("little-things-gb-dmg-extra")
}

pub fn little_things_gb_cgb_extra_suite() -> RomSuite {
    manifest_suite_by_name("little-things-gb-cgb-extra")
}

pub fn gbmicrotest_dmg_extra_suite() -> RomSuite {
    manifest_suite_by_name("gbmicrotest-dmg-extra")
}

pub fn docboy_dmg_extra_suite() -> RomSuite {
    manifest_suite_by_name("docboy-dmg-extra")
}

pub fn docboy_cgb_extra_suite() -> RomSuite {
    manifest_suite_by_name("docboy-cgb-extra")
}

pub fn docboy_cgb_dmg_extra_suite() -> RomSuite {
    manifest_suite_by_name("docboy-cgb-dmg-extra")
}

pub fn docboy_cgb_dmg_ext_extra_suite() -> RomSuite {
    manifest_suite_by_name("docboy-cgb-dmg-ext-extra")
}

pub fn blargg_dmg_curated_suite() -> RomSuite {
    manifest_suite("blargg")
}

pub fn blargg_dmg_repo_gated_suite() -> RomSuite {
    blargg_dmg_curated_suite()
}

pub fn blargg_dmg_cpu_instrs_suite() -> RomSuite {
    filtered_blargg_suite("blargg-dmg-cpu-instrs", |case| {
        case_rom_path_has_prefix(case, &["blargg", "cpu_instrs"])
    })
}

pub fn blargg_dmg_sound_suite() -> RomSuite {
    filtered_blargg_suite("blargg-dmg-sound", |case| {
        case_rom_path_has_prefix(case, &["blargg", "dmg_sound"])
    })
}

pub fn blargg_dmg_timing_memory_oam_suite() -> RomSuite {
    filtered_blargg_suite("blargg-dmg-timing-memory-oam", |case| {
        case_rom_path_has_prefix(case, &["blargg", "halt_bug.gb"])
            || case_rom_path_has_prefix(case, &["blargg", "instr_timing.gb"])
            || case_rom_path_has_prefix(case, &["blargg", "mem_timing"])
            || case_rom_path_has_prefix(case, &["blargg", "mem_timing-2"])
            || case_rom_path_has_prefix(case, &["blargg", "oam_bug"])
    })
}

pub fn blargg_dmg_curated_split_suites() -> Vec<RomSuite> {
    [
        blargg_dmg_cpu_instrs_suite(),
        blargg_dmg_sound_suite(),
        blargg_dmg_timing_memory_oam_suite(),
    ]
    .into()
}

fn filtered_blargg_suite(name: &str, include_case: impl Fn(&RomTestCase) -> bool) -> RomSuite {
    let mut suite = blargg_dmg_curated_suite();
    suite.name = name.to_string();
    suite.cases.retain(include_case);
    suite
}

pub fn daid_dmg_curated_suite() -> RomSuite {
    manifest_suite("daid")
}

pub fn hacktix_dmg_curated_suite() -> RomSuite {
    manifest_suite("hacktix")
}

pub fn cpp_dmg_curated_suite() -> RomSuite {
    manifest_suite("cpp")
}

pub fn mealybug_tearoom_dmg_curated_suite() -> RomSuite {
    manifest_suite("mealybug-tearoom-tests")
}

pub fn mealybug_tearoom_dmg_sameboy_differential_suite() -> RomSuite {
    let mut suite = mealybug_tearoom_dmg_curated_suite();
    suite.name = "mealybug-tearoom-dmg-sameboy-differential".to_string();
    suite
        .cases
        .retain(|case| !MEALYBUG_SAMEBOY_SHOOTOUT_NON_PASS_CASE_IDS.contains(&case.id.as_str()));
    suite
}

pub fn mooneye_acceptance_dmg_curated_suite() -> RomSuite {
    manifest_suite("mooneye")
}

pub fn mooneye_dmg_acceptance_manual_suite() -> RomSuite {
    filtered_mooneye_suite("mooneye-dmg-acceptance-manual", |case| {
        case_rom_path_has_prefix(case, &["mooneye", "acceptance"])
            || case_rom_path_has_prefix(case, &["mooneye", "manual-only"])
    })
}

pub fn mooneye_dmg_emulator_mbc1_mbc5_suite() -> RomSuite {
    filtered_mooneye_suite("mooneye-dmg-emulator-mbc1-mbc5", |case| {
        case_rom_path_has_prefix(case, &["mooneye", "emulator-only", "mbc1"])
            || case_rom_path_has_prefix(case, &["mooneye", "emulator-only", "mbc5"])
    })
}

pub fn mooneye_dmg_emulator_mbc2_suite() -> RomSuite {
    filtered_mooneye_suite("mooneye-dmg-emulator-mbc2", |case| {
        case_rom_path_has_prefix(case, &["mooneye", "emulator-only", "mbc2"])
    })
}

pub fn mooneye_dmg_curated_split_suites() -> Vec<RomSuite> {
    [
        mooneye_dmg_acceptance_manual_suite(),
        mooneye_dmg_emulator_mbc1_mbc5_suite(),
        mooneye_dmg_emulator_mbc2_suite(),
    ]
    .into()
}

fn filtered_mooneye_suite(name: &str, include_case: impl Fn(&RomTestCase) -> bool) -> RomSuite {
    let mut suite = mooneye_acceptance_dmg_curated_suite();
    suite.name = name.to_string();
    suite.cases.retain(include_case);
    suite
}

fn case_rom_path_has_prefix(case: &RomTestCase, prefix: &[&str]) -> bool {
    let mut components = case.rom_path.components();
    prefix.iter().all(|segment| {
        components
            .next()
            .is_some_and(|component| component.as_os_str() == std::ffi::OsStr::new(segment))
    })
}

pub fn curated_test_rom_family_suites() -> Vec<RomSuite> {
    [
        acid_dmg_curated_suite(),
        blargg_dmg_curated_suite(),
        cpp_dmg_curated_suite(),
        daid_dmg_curated_suite(),
        hacktix_dmg_curated_suite(),
        mealybug_tearoom_dmg_curated_suite(),
        mooneye_acceptance_dmg_curated_suite(),
    ]
    .into()
}

pub fn curated_test_rom_families() -> Vec<String> {
    let families = curated_test_rom_manifests()
        .into_iter()
        .flat_map(|manifest| manifest.cases.into_iter().map(|case| case.family))
        .collect::<BTreeSet<_>>();
    families.into_iter().collect()
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
        Some(GBEMU_SHOOTOUT_SOURCE_ID),
        selected_families,
    )?;
    materialize_curated_test_rom_source_filtered(
        workspace_root,
        Some(GBEMU_SHOOTOUT_SOURCE_ID),
        source_root,
        selected_families,
    )
}

pub(crate) fn replace_curated_test_rom_families(
    workspace_root: &Path,
    selected_families: &[String],
) -> Result<(), String> {
    let selected_families = selected_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    replace_curated_test_rom_family_roots(workspace_root, None, Some(&selected_families))
}

fn replace_curated_test_rom_family_roots(
    workspace_root: &Path,
    source_id: Option<&str>,
    selected_families: Option<&BTreeSet<&str>>,
) -> Result<(), String> {
    let store_root = test_rom_store_root(workspace_root);
    fs::create_dir_all(&store_root).map_err(|error| {
        format!(
            "failed to create curated test ROM store {}: {error}",
            store_root.display()
        )
    })?;

    let selected_cases_by_family =
        curated_test_rom_cases_by_family_from_source(selected_families, source_id);
    let mut materialized_families = BTreeSet::new();
    for family in selected_cases_by_family.keys() {
        materialized_families.insert(family.clone());
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

pub(crate) fn materialize_curated_test_rom_source_families(
    workspace_root: &Path,
    source_id: &str,
    source_root: &Path,
    selected_families: &[String],
) -> Result<(), String> {
    let selected_families = selected_families
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    materialize_curated_test_rom_source_filtered(
        workspace_root,
        Some(source_id),
        source_root,
        Some(&selected_families),
    )
}

fn materialize_curated_test_rom_source_filtered(
    workspace_root: &Path,
    source_id: Option<&str>,
    source_root: &Path,
    selected_families: Option<&BTreeSet<&str>>,
) -> Result<(), String> {
    let store_root = test_rom_store_root(workspace_root);
    let mut copied_targets = BTreeSet::new();
    for (_, cases) in curated_test_rom_cases_by_family_from_source(selected_families, source_id) {
        for case in cases.into_values() {
            let target_path =
                store_root.join(curated_case_store_relative_path(&case.family, &case.rom));
            copy_curated_source_rom(source_root, &case.source_path, &target_path)?;
            copied_targets.insert(target_path);
        }
    }
    if let Some(source_id) = source_id {
        for (family, source_path, rom) in
            curated_explicit_required_roms_for_source(source_id, selected_families)
        {
            let target_path = store_root.join(curated_case_store_relative_path(&family, &rom));
            if copied_targets.insert(target_path.clone()) {
                copy_curated_source_rom(source_root, &source_path, &target_path)?;
            }
        }
    }
    Ok(())
}

fn curated_explicit_required_roms_for_source(
    source_id: &str,
    selected_families: Option<&BTreeSet<&str>>,
) -> Vec<(String, PathBuf, PathBuf)> {
    let parsed: CuratedSourceManifestFile = toml::from_str(include_str!("../data/sources.toml"))
        .unwrap_or_else(|error| panic!("failed to parse curated source manifest: {error}"));
    parsed
        .sources
        .into_iter()
        .filter(|source| source.id == source_id)
        .flat_map(|source| source.required_files)
        .filter_map(|file| {
            let path = file.path;
            if !matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("gb" | "gbc")
            ) {
                return None;
            }
            let family = file.family?;
            if let Some(selected_families) = selected_families
                && !selected_families.contains(family.as_str())
            {
                return None;
            }
            let rom = file.rom?;
            Some((family, path, rom))
        })
        .collect()
}

fn curated_test_rom_cases_by_family_from_source(
    selected_families: Option<&BTreeSet<&str>>,
    source_id: Option<&str>,
) -> BTreeMap<String, BTreeMap<PathBuf, CuratedTestRomCase>> {
    let mut cases_by_family = BTreeMap::<String, BTreeMap<PathBuf, CuratedTestRomCase>>::new();
    for manifest in curated_test_rom_manifests() {
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

    let store_root = test_rom_store_root(workspace_root);
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
        version: CURATED_TEST_ROM_REPORT_VERSION,
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
        if !suite_status_path_matches_suite(&path, &persisted.suite_name) {
            continue;
        }
        push_or_merge_persisted_suite_status(
            &mut suites,
            normalize_persisted_suite_status(persisted),
        );
    }

    suites.sort_by(compare_report_suites);

    let standard_suites = report_suites_for_kind(&suites, CuratedTestReportKind::Standard);
    let extra_suites = report_suites_for_kind(&suites, CuratedTestReportKind::Extra);
    let docboy_suites = report_suites_for_kind(&suites, CuratedTestReportKind::DocBoy);
    let standard_report_path = write_markdown_report_file_if_needed(
        &store_root,
        TEST_ROM_REPORT_FILE_NAME,
        &standard_suites,
    )?;
    let extra_report_path = write_markdown_report_file_if_needed(
        &store_root,
        TEST_ROM_EXTRA_REPORT_FILE_NAME,
        &extra_suites,
    )?;
    let docboy_report_path = write_markdown_report_file_if_needed(
        &store_root,
        TEST_ROM_DOCBOY_REPORT_FILE_NAME,
        &docboy_suites,
    )?;

    let report_path = if suite_uses_status_only_test_report(&report.suite_name) {
        None
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
}

fn report_suites_for_kind(
    suites: &[PersistedSuiteStatus],
    report_kind: CuratedTestReportKind,
) -> Vec<PersistedSuiteStatus> {
    suites
        .iter()
        .filter(|suite| {
            !suite_uses_status_only_test_report(&suite.suite_name)
                && suite_test_report_kind(&suite.suite_name) == report_kind
        })
        .cloned()
        .collect()
}

fn suite_uses_extra_test_report(suite_name: &str) -> bool {
    EXTRA_CURATED_TEST_ROM_REPORT_SUITE_NAMES.contains(&suite_name)
}

fn suite_uses_docboy_test_report(suite_name: &str) -> bool {
    DOCBOY_CURATED_TEST_ROM_REPORT_SUITE_NAMES.contains(&suite_name)
}

fn suite_test_report_kind(suite_name: &str) -> CuratedTestReportKind {
    if suite_uses_docboy_test_report(suite_name) {
        CuratedTestReportKind::DocBoy
    } else if suite_uses_extra_test_report(suite_name) {
        CuratedTestReportKind::Extra
    } else {
        CuratedTestReportKind::Standard
    }
}

fn suite_uses_status_only_test_report(suite_name: &str) -> bool {
    STATUS_ONLY_CURATED_TEST_ROM_REPORT_SUITE_NAMES.contains(&suite_name)
}

fn write_markdown_report_file(
    store_root: &Path,
    file_name: &str,
    suites: &[PersistedSuiteStatus],
) -> Result<PathBuf, String> {
    let report_path = store_root.join(file_name);
    fs::write(&report_path, render_markdown_report(suites)).map_err(|error| {
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
) -> Result<Option<PathBuf>, String> {
    if suites.is_empty() {
        let report_path = store_root.join(file_name);
        remove_markdown_report_file_if_present(&report_path)?;
        return Ok(None);
    }

    write_markdown_report_file(store_root, file_name, suites).map(Some)
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
        "docboy-dmg-extra" => "docboy-dmg",
        "docboy-cgb-extra" => "docboy-cgb",
        "docboy-cgb-dmg-extra" => "docboy-cgb-dmg",
        "docboy-cgb-dmg-ext-extra" => "docboy-cgb-dmg-ext",
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
        .filter(|case| case.family == family)
        .enumerate()
    {
        if persisted_case_matches_manifest_case(family, rom, case) {
            let source_order = curated_source_rom_order(&case.family, &case.rom);
            return Some(ReportCaseOrder {
                source_order_missing: source_order.is_none(),
                source_or_manifest_order: source_order.unwrap_or(case_manifest_order),
                console_order: console_report_order(case.console_model),
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
        .filter(|case| case.family == family)
        .enumerate()
    {
        if persisted_case_matches_manifest_case(family, rom, case) {
            let source_order = curated_source_rom_order(&case.family, &case.rom);
            return Some(ReportCaseOrder {
                source_order_missing: source_order.is_none(),
                source_or_manifest_order: source_order.unwrap_or(case_manifest_order),
                console_order: console_report_order(case.console_model),
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
    case_statuses.sort_by_cached_key(|entry| {
        let entry_family = entry.family.as_deref().unwrap_or(family);
        let rank = report_family_rank(entry_family);
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

    let mut suite = RomSuite::new(manifest.suite_name, manifest.subsystem).with_family(family);
    for case in manifest.cases {
        if case.disabled {
            continue;
        }
        suite.push_case(manifest_case_to_rom_test_case(case));
    }
    suite
}

pub fn cgb_smoke_suite() -> RomSuite {
    manifest_suite_by_name("cgb-smoke")
}

pub fn cgb_boot_div_suite() -> RomSuite {
    manifest_suite_by_name("cgb-boot-div")
}

pub fn cgb_boot_hwio_suite() -> RomSuite {
    manifest_suite_by_name("cgb-boot-hwio")
}

pub fn cgb_audio_blargg_suite() -> RomSuite {
    manifest_suite_by_name("cgb-audio-blargg")
}

pub fn cgb_audio_samesuite_suite() -> RomSuite {
    manifest_suite_by_name("cgb-audio-samesuite")
}

pub fn cgb_ppu_basic_suite() -> RomSuite {
    manifest_suite_by_name("cgb-ppu-basic")
}

pub fn cgb_ppu_hard_suite() -> RomSuite {
    manifest_suite_by_name("cgb-ppu-hard")
}

pub fn cgb_dma_suite() -> RomSuite {
    manifest_suite_by_name("cgb-dma")
}

pub fn cgb_rtc_suite() -> RomSuite {
    manifest_suite_by_name("cgb-rtc")
}

pub fn cgb_speed_suite() -> RomSuite {
    manifest_suite_by_name("cgb-speed")
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

    let mut suite =
        RomSuite::new(manifest.suite_name, manifest.subsystem).with_family(suite_family);
    for case in manifest.cases {
        if case.disabled {
            continue;
        }
        suite.push_case(manifest_case_to_rom_test_case(case));
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

fn curated_test_rom_manifest_texts() -> [(&'static str, &'static str); 28] {
    [
        (
            "crates/gb-test-runner/data/acid.toml",
            include_str!("../data/acid.toml"),
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
            "crates/gb-test-runner/data/gbmicrotest.toml",
            include_str!("../data/gbmicrotest.toml"),
        ),
        (
            "crates/gb-test-runner/data/docboy-dmg.toml",
            include_str!("../data/docboy-dmg.toml"),
        ),
        (
            "crates/gb-test-runner/data/docboy-cgb.toml",
            include_str!("../data/docboy-cgb.toml"),
        ),
        (
            "crates/gb-test-runner/data/docboy-cgb-dmg.toml",
            include_str!("../data/docboy-cgb-dmg.toml"),
        ),
        (
            "crates/gb-test-runner/data/docboy-cgb-dmg-ext.toml",
            include_str!("../data/docboy-cgb-dmg-ext.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-audio-blargg.toml",
            include_str!("../data/cgb-audio-blargg.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-audio-samesuite.toml",
            include_str!("../data/cgb-audio-samesuite.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-boot-div.toml",
            include_str!("../data/cgb-boot-div.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-boot-hwio.toml",
            include_str!("../data/cgb-boot-hwio.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-smoke.toml",
            include_str!("../data/cgb-smoke.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-ppu-basic.toml",
            include_str!("../data/cgb-ppu-basic.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-ppu-hard.toml",
            include_str!("../data/cgb-ppu-hard.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-rtc.toml",
            include_str!("../data/cgb-rtc.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-dma.toml",
            include_str!("../data/cgb-dma.toml"),
        ),
        (
            "crates/gb-test-runner/data/cgb-speed.toml",
            include_str!("../data/cgb-speed.toml"),
        ),
        (
            "crates/gb-test-runner/data/blargg.toml",
            include_str!("../data/blargg.toml"),
        ),
        (
            "crates/gb-test-runner/data/daid.toml",
            include_str!("../data/daid.toml"),
        ),
        (
            "crates/gb-test-runner/data/cpp.toml",
            include_str!("../data/cpp.toml"),
        ),
        (
            "crates/gb-test-runner/data/hacktix.toml",
            include_str!("../data/hacktix.toml"),
        ),
        (
            "crates/gb-test-runner/data/mealybug-tearoom-tests.toml",
            include_str!("../data/mealybug-tearoom-tests.toml"),
        ),
        (
            "crates/gb-test-runner/data/mooneye.toml",
            include_str!("../data/mooneye.toml"),
        ),
    ]
}

fn parse_manifest(source_path: &'static str, source_text: &'static str) -> CuratedTestRomManifest {
    let parsed: CuratedTestRomManifestFile = toml::from_str(source_text)
        .unwrap_or_else(|error| panic!("failed to parse {source_path}: {error}"));
    assert_eq!(
        parsed.version, CURATED_TEST_ROM_MANIFEST_VERSION,
        "unsupported curated test ROM manifest version in {source_path}"
    );

    CuratedTestRomManifest {
        suite_family: parsed.family.clone(),
        suite_name: parsed.suite_name,
        subsystem: parse_manifest_subsystem(source_path, &parsed.subsystem),
        cases: parsed
            .cases
            .into_iter()
            .map(|case| parse_manifest_case(source_path, parsed.family.as_deref(), case))
            .collect(),
    }
}

fn parse_manifest_case(
    source_path: &str,
    manifest_family: Option<&str>,
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
    let console_model = parse_manifest_console_model(
        source_path,
        &case.id,
        case.console.as_deref().unwrap_or("dmg"),
    );
    let revision = case
        .revision
        .as_deref()
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
        case.startup.as_deref().unwrap_or("skip-boot"),
    );
    let source_id = case
        .source_id
        .unwrap_or_else(|| GBEMU_SHOOTOUT_SOURCE_ID.to_string());
    let source_path = case.source_path.unwrap_or_else(|| {
        PathBuf::from(GBEMU_SHOOTOUT_TESTROMS_DIR)
            .join(&family)
            .join(&case.rom)
    });
    let timeout = parse_manifest_timeout(&source_path, case.timeout_frames, case.timeout_tcycles);
    let case_id = case.id;
    let comment = normalize_manifest_case_comment(
        Path::new(manifest_path),
        &case_id,
        case.disabled,
        case.comment,
    );

    CuratedTestRomCase {
        family,
        id: case_id.clone(),
        rom: case.rom,
        source_id,
        source_path: source_path.clone(),
        report_model_suffix: case.report_model_suffix.unwrap_or(false),
        report_label: case.report_label,
        timeout,
        oracle: case.oracle,
        expected: case.expected,
        fixture: case.fixture,
        fixtures: case.fixtures,
        check_interval_tcycles: case.check_interval_tcycles,
        check_at_tcycles: case.check_at_tcycles,
        memory: case
            .memory
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
            .collect(),
        stimuli: case
            .stimuli
            .into_iter()
            .map(|stimulus| parse_manifest_stimulus(&source_path, &case_id, stimulus))
            .collect(),
        console_model,
        revision,
        startup_mode,
        execution_mode: case.execution_mode,
        stop_condition: case.stop_condition,
        disabled: case.disabled,
        comment,
    }
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

fn parse_manifest_subsystem(source_path: &str, subsystem: &str) -> TestSubsystem {
    match subsystem {
        "Ppu" => TestSubsystem::Ppu,
        "Dma" => TestSubsystem::Dma,
        "Apu" => TestSubsystem::Apu,
        "Cartridge" => TestSubsystem::Cartridge,
        "CrossSubsystem" => TestSubsystem::CrossSubsystem,
        other => panic!("unsupported subsystem {other:?} in {source_path}"),
    }
}

fn parse_manifest_console_model(source_path: &str, case_id: &str, console: &str) -> ConsoleModel {
    match console {
        "game-boy" | "dmg0" | "dmg" => ConsoleModel::GameBoy,
        "pocket" | "mgb" => ConsoleModel::GameBoyPocket,
        "light" => ConsoleModel::GameBoyLight,
        "color" | "cgb" => ConsoleModel::GameBoyColor,
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

fn manifest_case_to_rom_test_case(case: CuratedTestRomCase) -> RomTestCase {
    let CuratedTestRomCase {
        family,
        id,
        rom,
        source_id: _,
        source_path: _,
        report_model_suffix: _,
        report_label: _,
        timeout,
        oracle,
        expected,
        fixture,
        fixtures,
        check_interval_tcycles,
        check_at_tcycles,
        memory,
        stimuli,
        console_model,
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
        "info-serial" => PassCondition::Informational(CaptureKind::Serial),
        "info-serial-hex" => PassCondition::Informational(CaptureKind::SerialHex),
        "info-memory-text-output" => PassCondition::Informational(CaptureKind::MemoryTextOutput),
        "info-blargg-console-text" => PassCondition::Informational(CaptureKind::BlarggConsoleText),
        "info-snapshot" => PassCondition::Informational(CaptureKind::Snapshot),
        "info-framebuffer" => PassCondition::Informational(CaptureKind::Framebuffer),
        "info-trace" => PassCondition::Informational(CaptureKind::Trace),
        "framebuffer-fixture" => PassCondition::FramebufferFixture(
            fixture.unwrap_or_else(|| panic!("missing fixture path for case {id}")),
        ),
        "framebuffer-fixture-until-match" => PassCondition::FramebufferFixtureUntilMatch {
            fixture_path: fixture.unwrap_or_else(|| panic!("missing fixture path for case {id}")),
            check_interval_tcycles: check_interval_tcycles.unwrap_or(100_000),
            check_at_tcycles,
        },
        "framebuffer-grayscale-fixture" => PassCondition::FramebufferGrayscaleFixture(
            fixture.unwrap_or_else(|| panic!("missing fixture path for case {id}")),
        ),
        "framebuffer-rgb555-fixture" => PassCondition::FramebufferRgb555Fixture(
            fixture.unwrap_or_else(|| panic!("missing fixture path for case {id}")),
        ),
        "framebuffer-rgb555-fixture-until-match" => {
            PassCondition::FramebufferRgb555FixtureUntilMatch {
                fixture_path: fixture
                    .unwrap_or_else(|| panic!("missing fixture path for case {id}")),
                check_interval_tcycles: check_interval_tcycles.unwrap_or(100_000),
                check_at_tcycles,
            }
        }
        "framebuffer-rgb555-grayscale-fixture" => PassCondition::FramebufferRgb555GrayscaleFixture(
            fixture.unwrap_or_else(|| panic!("missing fixture path for case {id}")),
        ),
        "framebuffer-fixture-set" => PassCondition::FramebufferFixtureSet(
            fixtures.unwrap_or_else(|| panic!("missing fixture paths for case {id}")),
        ),
        other => panic!("unsupported oracle {other:?} for case {id}"),
    };

    let capture_plan = capture_plan_for_pass_condition(&pass_condition);
    let failure_artifacts = failure_artifacts_for_pass_condition(&pass_condition);
    let mut rom_case = RomTestCase::new(
        id,
        curated_case_store_relative_path(&family, &rom),
        timeout,
        pass_condition,
    )
    .with_external_rom_root_key(TEST_ROM_ROOT_ENV_VAR)
    .with_console_model(console_model)
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
            .with_capture(*capture)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::FramebufferFixture(_)
        | PassCondition::FramebufferFixtureUntilMatch { .. }
        | PassCondition::FramebufferGrayscaleFixture(_)
        | PassCondition::FramebufferRgb555Fixture(_)
        | PassCondition::FramebufferRgb555FixtureUntilMatch { .. }
        | PassCondition::FramebufferRgb555GrayscaleFixture(_)
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
            .with_artifact(*capture)
            .with_artifact(CaptureKind::Snapshot),
        PassCondition::FramebufferFixture(_)
        | PassCondition::FramebufferFixtureUntilMatch { .. }
        | PassCondition::FramebufferGrayscaleFixture(_)
        | PassCondition::FramebufferRgb555Fixture(_)
        | PassCondition::FramebufferRgb555FixtureUntilMatch { .. }
        | PassCondition::FramebufferRgb555GrayscaleFixture(_)
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

fn render_markdown_report(suites: &[PersistedSuiteStatus]) -> String {
    let mut ordered_suites = suites
        .iter()
        .cloned()
        .map(normalize_persisted_suite_status)
        .collect::<Vec<_>>();
    ordered_suites.sort_by(compare_report_suites);
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
            let left_rank = report_family_rank(left_family);
            let right_rank = report_family_rank(right_family);
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

fn report_family_rank(family: &str) -> Option<usize> {
    CURATED_TEST_ROM_REPORT_FAMILY_ORDER
        .iter()
        .position(|known_family| *known_family == family)
}

fn compare_report_suites(
    left: &PersistedSuiteStatus,
    right: &PersistedSuiteStatus,
) -> std::cmp::Ordering {
    let left_rank = report_family_rank(&left.family);
    let right_rank = report_family_rank(&right.family);

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
    if case.report_model_suffix {
        format!("{rom} ({})", console_report_suffix(case.console_model))
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
    let parsed: CuratedSourceManifestFile = toml::from_str(include_str!("../data/sources.toml"))
        .unwrap_or_else(|error| panic!("failed to parse curated source manifest: {error}"));
    assert_eq!(
        parsed.version, CURATED_SOURCE_MANIFEST_VERSION,
        "unsupported curated source manifest version"
    );

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

fn console_report_suffix(console_model: ConsoleModel) -> &'static str {
    match console_model {
        ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => "DMG",
        ConsoleModel::GameBoyColor => "GBC",
    }
}

fn console_report_order(console_model: ConsoleModel) -> usize {
    match console_model {
        ConsoleModel::GameBoy | ConsoleModel::GameBoyPocket | ConsoleModel::GameBoyLight => 0,
        ConsoleModel::GameBoyColor => 1,
    }
}

fn report_rom_display(family: &str, rom_path: &Path) -> String {
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
    use super::{
        CuratedTestRomCase, CuratedTestRomCaseFile, CuratedTestRomManifestFile,
        GBEMU_SHOOTOUT_SOURCE_ID, MEALYBUG_SAMEBOY_SHOOTOUT_NON_PASS_CASE_IDS, PersistedCaseStatus,
        PersistedSuiteStatus, REPORT_STATUS_FAIL_EMOJI, REPORT_STATUS_INFO_EMOJI,
        REPORT_STATUS_PASS_EMOJI, TEST_ROM_DOCBOY_REPORT_FILE_NAME,
        TEST_ROM_EXTRA_REPORT_FILE_NAME, TEST_ROM_REPORT_FILE_NAME, TEST_ROM_ROOT_ENV_VAR,
        TEST_ROM_STATUS_DIR_NAME, ax6_dmg_extra_suite, blargg_dmg_curated_suite,
        blargg_dmg_repo_gated_suite, blargg_memory_text_output_spec,
        capture_plan_for_pass_condition, cgb_audio_blargg_suite, cgb_audio_samesuite_suite,
        cgb_boot_div_suite, cgb_boot_hwio_suite, cgb_dma_suite, cgb_ppu_basic_suite,
        cgb_ppu_hard_suite, cgb_rtc_suite, cgb_smoke_suite, copy_curated_rom,
        curated_test_rom_families, curated_test_rom_family_suites, curated_test_rom_manifest_texts,
        curated_test_rom_manifests, discover_test_rom_store_root, docboy_cgb_dmg_ext_extra_suite,
        docboy_cgb_dmg_extra_suite, docboy_cgb_extra_suite, docboy_dmg_extra_suite,
        failure_artifacts_for_pass_condition, gbmicrotest_dmg_extra_suite,
        little_things_gb_cgb_extra_suite, little_things_gb_dmg_extra_suite,
        load_persisted_suite_status, magen_cgb_extra_suite, manifest_case_report_rom_display,
        manifest_case_to_rom_test_case, materialize_curated_test_rom_families,
        materialize_curated_test_rom_store, mealybug_tearoom_dmg_curated_suite,
        mealybug_tearoom_dmg_sameboy_differential_suite, parse_manifest_case,
        parse_manifest_console_model, parse_manifest_subsystem, render_markdown_report,
        report_rom_display, report_status_display, samesuite_cgb_extra_suite,
        samesuite_dmg_extra_suite, sort_persisted_case_statuses, suite_uses_docboy_test_report,
        suite_uses_extra_test_report, test_rom_store_root, update_curated_test_report,
    };
    use crate::{
        CaptureKind, CapturedArtifacts, MemoryByteExpectation, PassCondition, RomCaseFailure,
        RomCaseOutcome, RomCaseReport, RomSuiteReport, TestSubsystem, Timeout,
    };
    use gb_core::{ConsoleModel, HardwareRevision, StartupMode};
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
        let acid_root = test_rom_store_root(&workspace_root).join("acid");
        fs::create_dir_all(acid_root.parent().expect("acid root should have a parent"))
            .expect("store root should be creatable");
        fs::write(&acid_root, "not-a-directory").expect("acid file should be writable");
        let error = materialize_curated_test_rom_families(
            &workspace_root,
            &gbemu_shootout_root,
            &["acid".to_string()],
        )
        .expect_err("family file should reject replacement");
        assert!(error.contains("failed to replace curated family directory"));

        let blocked_parent = workspace_root.join("blocked-parent");
        fs::write(&blocked_parent, "not-a-directory").expect("blocked parent should be writable");
        let source_rom = gbemu_shootout_root.join("testroms/acid/which.gb");
        assert!(source_rom.exists());
        let error = copy_curated_rom(
            &gbemu_shootout_root,
            "acid",
            Path::new("which.gb"),
            &blocked_parent.join("which.gb"),
        )
        .expect_err("file parent should reject copied ROM target");
        assert!(error.contains("failed to create curated ROM parent"));

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn curated_blargg_manifest_tracks_the_full_individual_shootout_list() {
        let suite = blargg_dmg_curated_suite();

        assert_eq!(suite.name, "blargg-dmg-curated");
        assert_eq!(suite.family.as_deref(), Some("blargg"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 38);
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "blargg-instr-timing")
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "blargg-dmg-sound-12-wave-write-while-on")
        );
        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.rom_path.starts_with(Path::new("blargg")))
        );
    }

    #[test]
    fn repo_gated_blargg_suite_now_matches_the_promoted_curated_family() {
        let suite = blargg_dmg_repo_gated_suite();

        assert_eq!(suite.name, "blargg-dmg-curated");
        assert_eq!(suite.family.as_deref(), Some("blargg"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 38);
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "blargg-dmg-sound-12-wave-write-while-on")
        );
    }

    #[test]
    fn sameboy_mealybug_differential_suite_excludes_shootout_non_pass_cases_only() {
        let full_suite = mealybug_tearoom_dmg_curated_suite();
        let sameboy_suite = mealybug_tearoom_dmg_sameboy_differential_suite();

        assert_eq!(full_suite.cases.len(), 24);
        assert_eq!(
            sameboy_suite.name,
            "mealybug-tearoom-dmg-sameboy-differential"
        );
        assert_eq!(
            sameboy_suite.family.as_deref(),
            Some("mealybug-tearoom-tests")
        );
        assert_eq!(
            sameboy_suite.cases.len(),
            full_suite.cases.len() - MEALYBUG_SAMEBOY_SHOOTOUT_NON_PASS_CASE_IDS.len()
        );
        for excluded_id in MEALYBUG_SAMEBOY_SHOOTOUT_NON_PASS_CASE_IDS {
            assert!(
                full_suite.cases.iter().any(|case| case.id == *excluded_id),
                "excluded id {excluded_id} should still exist in the full curated suite"
            );
            assert!(
                sameboy_suite
                    .cases
                    .iter()
                    .all(|case| case.id != *excluded_id),
                "excluded id {excluded_id} must not be judged in the SameBoy differential subset"
            );
        }
        assert!(
            sameboy_suite
                .cases
                .iter()
                .any(|case| case.id == "mealybug-m3-window-timing")
        );
    }

    #[test]
    fn curated_manifest_cases_declare_console_explicitly() {
        for (source_path, source_text) in curated_test_rom_manifest_texts() {
            let manifest: CuratedTestRomManifestFile = toml::from_str(source_text)
                .unwrap_or_else(|error| panic!("failed to parse {source_path}: {error}"));
            let missing_console = manifest
                .cases
                .iter()
                .filter(|case| case.console.is_none())
                .map(|case| case.id.as_str())
                .collect::<Vec<_>>();

            assert!(
                missing_console.is_empty(),
                "{source_path} cases missing explicit console: {missing_console:?}"
            );
        }
    }

    #[test]
    fn cgb_smoke_suite_is_manifest_backed_and_uses_upstream_families() {
        let suite = cgb_smoke_suite();

        assert_eq!(suite.name, "cgb-smoke");
        assert_eq!(suite.family.as_deref(), Some("cgb-smoke"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 2);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.startup_mode == StartupMode::SkipBoot
        }));
        assert_eq!(
            suite.cases[0].rom_path,
            PathBuf::from("mooneye/misc/boot_regs-cgb.gb")
        );
        assert!(matches!(
            suite.cases[0].pass_condition,
            PassCondition::MooneyeResult
        ));
        assert_eq!(suite.cases[1].rom_path, PathBuf::from("acid/which.gb"));
        assert!(matches!(
            suite.cases[1].pass_condition,
            PassCondition::Informational(CaptureKind::Framebuffer)
        ));
    }

    #[test]
    fn cgb_boot_div_suite_is_manifest_backed_and_uses_mooneye_result() {
        let suite = cgb_boot_div_suite();

        assert_eq!(suite.name, "cgb-boot-div");
        assert_eq!(suite.family.as_deref(), Some("cgb-boot-div"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 1);
        assert_eq!(
            suite.cases[0].rom_path,
            PathBuf::from("mooneye/misc/boot_div-cgbABCDE.gb")
        );
        assert_eq!(suite.cases[0].console_model, ConsoleModel::GameBoyColor);
        assert_eq!(suite.cases[0].startup_mode, StartupMode::SkipBoot);
        assert_eq!(
            suite.cases[0].external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert!(matches!(
            suite.cases[0].pass_condition,
            PassCondition::MooneyeResult
        ));
    }

    #[test]
    fn cgb_boot_hwio_suite_is_manifest_backed_and_internal_mooneye_gate() {
        let suite = cgb_boot_hwio_suite();

        assert_eq!(suite.name, "cgb-boot-hwio");
        assert_eq!(suite.family.as_deref(), Some("cgb-boot-hwio"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 1);
        assert_eq!(
            suite.cases[0].rom_path,
            PathBuf::from("mooneye/misc/boot_hwio-C.gb")
        );
        assert_eq!(suite.cases[0].console_model, ConsoleModel::GameBoyColor);
        assert_eq!(suite.cases[0].startup_mode, StartupMode::SkipBoot);
        assert_eq!(
            suite.cases[0].external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert!(matches!(
            suite.cases[0].pass_condition,
            PassCondition::MooneyeResult
        ));
    }

    #[test]
    fn cgb_ppu_basic_suite_promotes_initial_slice4_rows_in_order() {
        let suite = cgb_ppu_basic_suite();

        assert_eq!(suite.name, "cgb-ppu-basic");
        assert_eq!(suite.family.as_deref(), Some("cgb-ppu-basic"));
        assert_eq!(suite.subsystem, TestSubsystem::Ppu);
        assert_eq!(suite.cases.len(), 4);

        let case = &suite.cases[0];
        assert_eq!(case.id, "cgb-ppu-basic-blocking-bgpi-increase");
        assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
        assert_eq!(
            case.rom_path,
            PathBuf::from("samesuite/ppu/blocking_bgpi_increase.gb")
        );
        assert_eq!(
            case.external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert_eq!(
            case.pass_condition,
            PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                "crates/gb-test-runner/data/fixtures/samesuite/ppu/blocking_bgpi_increase.png"
            ))
        );

        let case = &suite.cases[1];
        assert_eq!(case.id, "cgb-ppu-basic-ppu-scanline-bgp-gbc");
        assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
        assert_eq!(case.rom_path, PathBuf::from("daid/ppu_scanline_bgp.gb"));
        assert_eq!(
            case.external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert_eq!(
            case.pass_condition,
            PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                "crates/gb-test-runner/data/fixtures/daid/ppu_scanline_bgp.gbc.png"
            ))
        );

        let case = &suite.cases[2];
        assert_eq!(case.id, "cgb-ppu-basic-cgb-acid2");
        assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
        assert_eq!(case.rom_path, PathBuf::from("acid/cgb-acid2.gbc"));
        assert_eq!(
            case.external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert_eq!(
            case.pass_condition,
            PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                "crates/gb-test-runner/data/fixtures/acid/cgb-acid2-cgb.png"
            ))
        );

        let case = &suite.cases[3];
        assert_eq!(case.id, "cgb-ppu-basic-hacktix-bully-gbc");
        assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
        assert_eq!(case.rom_path, PathBuf::from("hacktix/bully.gb"));
        assert_eq!(
            case.external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert_eq!(
            case.pass_condition,
            PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                "crates/gb-test-runner/data/fixtures/hacktix/bully.cgb.png"
            ))
        );
        assert_eq!(case.startup_timer_state, None);
        assert_eq!(case.startup_mode, StartupMode::CustomBoot);
        assert!(case.startup_memory_writes.is_empty());
    }

    #[test]
    fn cgb_ppu_hard_suite_promotes_cgb_acid_hell_closure_row() {
        let suite = cgb_ppu_hard_suite();

        assert_eq!(suite.name, "cgb-ppu-hard");
        assert_eq!(suite.family.as_deref(), Some("acid"));
        assert_eq!(suite.subsystem, TestSubsystem::Ppu);
        assert_eq!(suite.cases.len(), 1);

        let case = &suite.cases[0];
        assert_eq!(case.id, "cgb-ppu-hard-cgb-acid-hell");
        assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
        assert_eq!(case.rom_path, PathBuf::from("acid/cgb-acid-hell.gbc"));
        assert_eq!(case.timeout, Timeout::Frames(180));
        assert_eq!(
            case.external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert_eq!(
            case.pass_condition,
            PassCondition::FramebufferRgb555Fixture(PathBuf::from(
                "crates/gb-test-runner/data/fixtures/acid/cgb-acid-hell.png"
            ))
        );
        assert_eq!(case.stop_condition, None);
    }

    #[test]
    fn cgb_dma_suite_promotes_slice5_rows_to_blocking_oracles() {
        let suite = cgb_dma_suite();

        assert_eq!(suite.name, "cgb-dma");
        assert_eq!(suite.family.as_deref(), Some("cgb-dma"));
        assert_eq!(suite.subsystem, TestSubsystem::Dma);
        assert_eq!(suite.cases.len(), 4);

        let expected = [
            (
                "cgb-dma-gbc-dma-cont",
                "samesuite/dma/gbc_dma_cont.gb",
                "crates/gb-test-runner/data/fixtures/samesuite/dma/gbc_dma_cont.png",
            ),
            (
                "cgb-dma-gdma-addr-mask",
                "samesuite/dma/gdma_addr_mask.gb",
                "crates/gb-test-runner/data/fixtures/samesuite/dma/gdma_addr_mask.png",
            ),
            (
                "cgb-dma-hdma-lcd-off",
                "samesuite/dma/hdma_lcd_off.gb",
                "crates/gb-test-runner/data/fixtures/samesuite/dma/hdma_lcd_off.png",
            ),
            (
                "cgb-dma-hdma-mode0",
                "samesuite/dma/hdma_mode0.gb",
                "crates/gb-test-runner/data/fixtures/samesuite/dma/hdma_mode0.png",
            ),
        ];

        for (case, (id, rom_path, fixture_path)) in suite.cases.iter().zip(expected) {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
            assert_eq!(case.rom_path, PathBuf::from(rom_path));
            assert_eq!(
                case.external_rom_root_key.as_deref(),
                Some(TEST_ROM_ROOT_ENV_VAR)
            );
            assert_eq!(
                case.pass_condition,
                PassCondition::FramebufferRgb555Fixture(PathBuf::from(fixture_path))
            );
        }
    }

    #[test]
    fn cgb_rtc_suite_promotes_slice8_ax6_rows_to_blocking_oracles() {
        let suite = cgb_rtc_suite();

        assert_eq!(suite.name, "cgb-rtc");
        assert_eq!(suite.family.as_deref(), Some("cgb-rtc"));
        assert_eq!(suite.subsystem, TestSubsystem::Cartridge);
        assert_eq!(suite.cases.len(), 3);

        let expected = [
            (
                "cgb-rtc-rtc3test-1",
                "ax6/rtc3test-1.gb",
                Timeout::Frames(1140),
                "crates/gb-test-runner/data/fixtures/ax6/rtc3test-1.png",
            ),
            (
                "cgb-rtc-rtc3test-2",
                "ax6/rtc3test-2.gb",
                Timeout::Frames(900),
                "crates/gb-test-runner/data/fixtures/ax6/rtc3test-2.png",
            ),
            (
                "cgb-rtc-rtc3test-3",
                "ax6/rtc3test-3.gb",
                Timeout::Frames(2400),
                "crates/gb-test-runner/data/fixtures/ax6/rtc3test-3.png",
            ),
        ];

        for (case, (id, rom_path, timeout, fixture_path)) in suite.cases.iter().zip(expected) {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
            assert_eq!(case.rom_path, PathBuf::from(rom_path));
            assert_eq!(case.timeout, timeout);
            assert_eq!(
                case.external_rom_root_key.as_deref(),
                Some(TEST_ROM_ROOT_ENV_VAR)
            );
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
        assert_eq!(suite.subsystem, TestSubsystem::Cartridge);
        assert_eq!(suite.cases.len(), 3);

        let expected = [
            (
                "ax6-dmg-rtc3test-1",
                "ax6/rtc3test-1.gb",
                Timeout::Frames(1140),
                "crates/gb-test-runner/data/fixtures/ax6/rtc3test-1.dmg.png",
                "rtc3test-1.gb (DMG)",
            ),
            (
                "ax6-dmg-rtc3test-2",
                "ax6/rtc3test-2.gb",
                Timeout::Frames(900),
                "crates/gb-test-runner/data/fixtures/ax6/rtc3test-2.dmg.png",
                "rtc3test-2.gb (DMG)",
            ),
            (
                "ax6-dmg-rtc3test-3",
                "ax6/rtc3test-3.gb",
                Timeout::Frames(2400),
                "crates/gb-test-runner/data/fixtures/ax6/rtc3test-3.dmg.png",
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
            assert_eq!(case.rom_path, PathBuf::from(rom_path));
            assert_eq!(case.timeout, timeout);
            assert_eq!(
                case.external_rom_root_key.as_deref(),
                Some(TEST_ROM_ROOT_ENV_VAR)
            );
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
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 3);

        let expected = [
            (
                "samesuite-dmg-div-write-trigger",
                "samesuite/apu/div_write_trigger.gb",
                "crates/gb-test-runner/data/fixtures/samesuite/apu/div_write_trigger.png",
                "apu/div_write_trigger.gb (DMG)",
            ),
            (
                "samesuite-dmg-div-write-trigger-10",
                "samesuite/apu/div_write_trigger_10.gb",
                "crates/gb-test-runner/data/fixtures/samesuite/apu/div_write_trigger_10.png",
                "apu/div_write_trigger_10.gb (DMG)",
            ),
            (
                "samesuite-dmg-ei-delay-halt",
                "samesuite/interrupt/ei_delay_halt.gb",
                "crates/gb-test-runner/data/fixtures/samesuite/interrupt/ei_delay_halt.png",
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
            assert_eq!(case.rom_path, PathBuf::from(rom_path));
            assert_eq!(case.timeout, Timeout::Frames(180));
            assert_eq!(
                case.external_rom_root_key.as_deref(),
                Some(TEST_ROM_ROOT_ENV_VAR)
            );
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
    fn little_things_gb_dmg_extra_suite_forces_dmg_model_and_boot_logo_seed() {
        let suite = little_things_gb_dmg_extra_suite();

        assert_eq!(suite.name, "little-things-gb-dmg-extra");
        assert_eq!(suite.family.as_deref(), Some("little-things-gb"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 2);

        let expected = [
            (
                "little-things-gb-dmg-double-halt-cancel",
                "little-things-gb/double-halt-cancel.gb",
                "crates/gb-test-runner/data/fixtures/little-things-gb/double-halt-cancel.png",
                "double-halt-cancel.gb",
                StartupMode::SkipBoot,
            ),
            (
                "little-things-gb-dmg-whichboot",
                "little-things-gb/whichboot.gb",
                "crates/gb-test-runner/data/fixtures/little-things-gb/whichboot.png",
                "whichboot.gb",
                StartupMode::CustomBoot,
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
            assert_eq!(case.rom_path, PathBuf::from(rom_path));
            assert_eq!(case.timeout, Timeout::Frames(180));
            assert_eq!(
                case.external_rom_root_key.as_deref(),
                Some(TEST_ROM_ROOT_ENV_VAR)
            );
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
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 1);

        let case = &suite.cases[0];
        assert_eq!(case.id, "little-things-gb-cgb-whichboot");
        assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
        assert_eq!(
            case.rom_path,
            PathBuf::from("little-things-gb/whichboot.gb")
        );
        assert_eq!(case.timeout, Timeout::Frames(180));
        assert_eq!(
            case.external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert_eq!(
            case.pass_condition,
            PassCondition::FramebufferFixture(PathBuf::from(
                "crates/gb-test-runner/data/fixtures/little-things-gb-cgb/whichboot.png"
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
        assert!(manifest.cases[0].report_model_suffix);
        assert_eq!(
            manifest_case_report_rom_display(&manifest.cases[0]),
            "whichboot.gb (GBC)"
        );
        assert!(suite_uses_extra_test_report("little-things-gb-cgb-extra"));
        assert!(!suite_uses_docboy_test_report("little-things-gb-cgb-extra"));
    }

    #[test]
    fn gbmicrotest_dmg_extra_suite_tracks_docboy_memory_oracles_with_custom_boot_poweron_rows() {
        let manifest_text = include_str!("../data/gbmicrotest.toml");
        assert!(
            manifest_text.matches("startup = \"custom-boot\"").count() == 62,
            "gbmicrotest should use CustomBoot only for reset-facing poweron rows"
        );
        assert!(
            !manifest_text.contains("startup_ppu_profile"),
            "gbmicrotest should rely on core CustomBoot PPU publication instead of runner profiles"
        );

        let suite = gbmicrotest_dmg_extra_suite();

        assert_eq!(suite.name, "gbmicrotest-dmg-extra");
        assert_eq!(suite.family.as_deref(), Some("gbmicrotest"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 438);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoy
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.capture_plan.contains(CaptureKind::MemoryBytes)
                && case.capture_plan.contains(CaptureKind::Snapshot)
                && case.failure_artifacts.contains(CaptureKind::MemoryBytes)
                && case.failure_artifacts.contains(CaptureKind::Snapshot)
                && case.rom_path.starts_with("gbmicrotest")
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
                    case.rom_path
                        .to_string_lossy()
                        .starts_with("gbmicrotest/boot/poweron_")
                        && case.startup_mode == StartupMode::CustomBoot
                })
                .count(),
            62
        );
        assert!(suite.cases.iter().all(|case| {
            case.startup_mode
                == if case
                    .rom_path
                    .to_string_lossy()
                    .starts_with("gbmicrotest/boot/poweron_")
                {
                    StartupMode::CustomBoot
                } else {
                    StartupMode::SkipBoot
                }
        }));
        let dma_rows = [
            "gbmicrotest/dma/dma_0x1000.gb",
            "gbmicrotest/dma/dma_0x9000.gb",
            "gbmicrotest/dma/dma_0xA000.gb",
            "gbmicrotest/dma/dma_0xC000.gb",
            "gbmicrotest/dma/dma_0xE000.gb",
            "gbmicrotest/dma/dma_timing_a.gb",
        ];
        for rom_path in dma_rows {
            assert!(
                suite
                    .cases
                    .iter()
                    .any(|case| case.rom_path == Path::new(rom_path)),
                "{rom_path} should be materialized from DocBoy's on-disk gbmicrotest/dma ROMs"
            );
        }
        let long_spin_if_ime0 = suite
            .cases
            .iter()
            .find(|case| case.id == "gbmicrotest-interrupts-is-if-set-during-ime0")
            .expect("long IME=0 IF visibility row should stay in the DocBoy manifest");
        assert_eq!(long_spin_if_ime0.timeout, Timeout::TCycles(2_000_000));
        assert!(suite.cases.iter().all(|case| {
            case.id == long_spin_if_ime0.id || case.timeout == Timeout::TCycles(1_000_000)
        }));
        assert!(suite_uses_extra_test_report("gbmicrotest-dmg-extra"));
    }

    #[test]
    fn docboy_dmg_extra_suite_tracks_single_machine_docboy_rows() {
        let manifest_text = include_str!("../data/docboy-dmg.toml");
        assert!(
            !manifest_text.contains("startup ="),
            "docboy manifest must stay startup-neutral so Make targets choose SkipBoot or RealBoot"
        );
        assert!(
            !manifest_text.contains("startup_ppu_profile"),
            "docboy-dmg should not rely on runner-only PPU profiles"
        );

        let suite = docboy_dmg_extra_suite();

        assert_eq!(suite.name, "docboy-dmg-extra");
        assert_eq!(suite.family.as_deref(), Some("docboy-dmg"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 2326);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoy
                && case.startup_mode == StartupMode::SkipBoot
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.rom_path.starts_with("docboy/dmg")
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
        assert!(suite_uses_docboy_test_report("docboy-dmg-extra"));
        assert!(!suite_uses_extra_test_report("docboy-dmg-extra"));
    }

    #[test]
    fn docboy_cgb_extra_suite_tracks_native_cgb_docboy_rows() {
        let manifest_text = include_str!("../data/docboy-cgb.toml");
        assert!(
            !manifest_text.contains("startup ="),
            "DocBoy CGB manifest must stay startup-neutral so Make targets choose SkipBoot or RealBoot"
        );

        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "docboy-cgb-extra")
            .expect("DocBoy CGB manifest should exist");
        assert_eq!(manifest.suite_family.as_deref(), Some("docboy-cgb"));
        assert_eq!(manifest.cases.len(), 6815);
        assert_eq!(
            manifest.cases.iter().filter(|case| case.disabled).count(),
            643
        );

        let suite = docboy_cgb_extra_suite();

        assert_eq!(suite.name, "docboy-cgb-extra");
        assert_eq!(suite.family.as_deref(), Some("docboy-cgb"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 6172);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.rom_path.starts_with("docboy/cgb")
        }));
        assert!(
            suite
                .cases
                .iter()
                .all(|case| !case.rom_path.starts_with("docboy/cgb/blargg/cgb_sound"))
        );
        assert!(
            suite
                .cases
                .iter()
                .all(|case| !case.rom_path.starts_with("docboy/cgb/samesuite"))
        );
        assert!(
            suite
                .cases
                .iter()
                .all(|case| !case.rom_path.starts_with("docboy/cgb/magen"))
        );
        assert!(
            suite
                .cases
                .iter()
                .all(|case| !case.rom_path.starts_with("docboy/cgb/daid"))
        );
        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.rom_path != Path::new("docboy/cgb/mattcurrie/cgb-acid2.gbc"))
        );
        assert!(suite.cases.iter().all(|case| {
            case.rom_path != Path::new("docboy/cgb/little-things-gb/whichboot.gb")
        }));
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
                && case.rom_path.as_path()
                    == Path::new("docboy/cgb/ppu/visual/stop_ly42_during_hblank.gbc")
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
        assert!(suite_uses_docboy_test_report("docboy-cgb-extra"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-extra"));
    }

    #[test]
    fn docboy_cgb_dmg_extra_suite_tracks_compatibility_mode_docboy_rows() {
        let manifest_text = include_str!("../data/docboy-cgb-dmg.toml");
        assert!(
            !manifest_text.contains("startup ="),
            "DocBoy CGB DMG manifest must stay startup-neutral so Make targets choose SkipBoot or RealBoot"
        );

        let manifest = curated_test_rom_manifests()
            .into_iter()
            .find(|manifest| manifest.suite_name == "docboy-cgb-dmg-extra")
            .expect("DocBoy CGB DMG manifest should exist");
        assert_eq!(manifest.suite_family.as_deref(), Some("docboy-cgb-dmg"));
        assert_eq!(manifest.cases.len(), 504);
        assert_eq!(
            manifest.cases.iter().filter(|case| case.disabled).count(),
            1
        );
        assert!(manifest.cases.iter().any(|case| {
            case.id == "docboy-cgb-dmg-mealybug-m3-lcdc-win-en-change-multiple-wx"
                && case.disabled
                && case.comment.as_deref() == Some("Different from DMG: SameBoy is wrong as well")
        }));

        let suite = docboy_cgb_dmg_extra_suite();

        assert_eq!(suite.name, "docboy-cgb-dmg-extra");
        assert_eq!(suite.family.as_deref(), Some("docboy-cgb-dmg"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 503);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.rom_path.starts_with("docboy/cgb-dmg")
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
            435
        );
        assert!(suite.cases.iter().all(|case| {
            case.id != "docboy-cgb-dmg-mealybug-m3-lcdc-win-en-change-multiple-wx"
        }));
        assert_eq!(
            suite
                .cases
                .iter()
                .filter(|case| case.rom_path == Path::new("docboy/cgb-dmg/docboy/boot/boot_vram.gb"))
                .count(),
            1,
            "DocBoy cgb_dmg_mode.json carries one exact boot_vram duplicate that should not create duplicate runnable rows"
        );
        assert!(suite.cases.iter().any(|case| {
            case.id == "docboy-cgb-dmg-mode-mode-cgb-flag-84"
                && case.rom_path.as_path()
                    == Path::new("docboy/cgb-dmg/docboy/mode/mode_cgb_flag_84.gb")
                && case.pass_condition
                    == PassCondition::MemoryBytesEqual(vec![
                        MemoryByteExpectation::with_fail_value(0xFFF0, 0x01, 0x02),
                    ])
        }));
        assert!(suite_uses_docboy_test_report("docboy-cgb-dmg-extra"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-dmg-extra"));
    }

    #[test]
    fn docboy_cgb_dmg_ext_extra_suite_tracks_mixed_strict_and_experimental_docboy_rows() {
        let manifest_text = include_str!("../data/docboy-cgb-dmg-ext.toml");
        assert!(
            !manifest_text.contains("startup ="),
            "DocBoy CGB DMG-ext manifest must stay startup-neutral so Make targets choose SkipBoot or RealBoot"
        );

        let suite = docboy_cgb_dmg_ext_extra_suite();

        assert_eq!(suite.name, "docboy-cgb-dmg-ext-extra");
        assert_eq!(suite.family.as_deref(), Some("docboy-cgb-dmg-ext"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 26);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.rom_path.starts_with("docboy/cgb-dmg-ext")
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
                && case.rom_path.as_path()
                    == Path::new("docboy/cgb-dmg-ext/mode/mode_cgb_flag_8c.gb")
        }));
        assert!(suite_uses_docboy_test_report("docboy-cgb-dmg-ext-extra"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-dmg-ext-extra"));
    }

    #[test]
    fn cgb_audio_blargg_suite_tracks_the_full_cgb_sound_lane() {
        let suite = cgb_audio_blargg_suite();

        assert_eq!(suite.name, "cgb-audio-blargg");
        assert_eq!(suite.family.as_deref(), Some("cgb-audio-blargg"));
        assert_eq!(suite.subsystem, TestSubsystem::Apu);
        assert_eq!(suite.cases.len(), 12);

        let expected = [
            (
                "cgb-audio-blargg-01-registers",
                "blargg/cgb_sound/01-registers.gb",
            ),
            (
                "cgb-audio-blargg-02-len-ctr",
                "blargg/cgb_sound/02-len_ctr.gb",
            ),
            (
                "cgb-audio-blargg-03-trigger",
                "blargg/cgb_sound/03-trigger.gb",
            ),
            ("cgb-audio-blargg-04-sweep", "blargg/cgb_sound/04-sweep.gb"),
            (
                "cgb-audio-blargg-05-sweep-details",
                "blargg/cgb_sound/05-sweep_details.gb",
            ),
            (
                "cgb-audio-blargg-06-overflow-on-trigger",
                "blargg/cgb_sound/06-overflow_on_trigger.gb",
            ),
            (
                "cgb-audio-blargg-07-len-sweep-period-sync",
                "blargg/cgb_sound/07-len_sweep_period_sync.gb",
            ),
            (
                "cgb-audio-blargg-08-len-ctr-during-power",
                "blargg/cgb_sound/08-len_ctr_during_power.gb",
            ),
            (
                "cgb-audio-blargg-09-wave-read-while-on",
                "blargg/cgb_sound/09-wave_read_while_on.gb",
            ),
            (
                "cgb-audio-blargg-10-wave-trigger-while-on",
                "blargg/cgb_sound/10-wave_trigger_while_on.gb",
            ),
            (
                "cgb-audio-blargg-11-regs-after-power",
                "blargg/cgb_sound/11-regs_after_power.gb",
            ),
            ("cgb-audio-blargg-12-wave", "blargg/cgb_sound/12-wave.gb"),
        ];

        for (case, (id, rom_path)) in suite.cases.iter().zip(expected) {
            assert_eq!(case.id, id);
            assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
            assert_eq!(case.rom_path, PathBuf::from(rom_path));
            assert_eq!(
                case.external_rom_root_key.as_deref(),
                Some(TEST_ROM_ROOT_ENV_VAR)
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
    fn cgb_audio_samesuite_suite_tracks_the_promoted_apu_lane() {
        let suite = cgb_audio_samesuite_suite();

        assert_eq!(suite.name, "cgb-audio-samesuite");
        assert_eq!(suite.family.as_deref(), Some("cgb-audio-samesuite"));
        assert_eq!(suite.subsystem, TestSubsystem::Apu);
        assert_eq!(suite.cases.len(), 61);

        let first = suite.cases.first().expect("suite should have cases");
        assert_eq!(first.id, "cgb-audio-samesuite-channel-1-align");
        assert_eq!(
            first.rom_path,
            PathBuf::from("samesuite/apu/channel_1/channel_1_align.gb")
        );

        let last = suite.cases.last().expect("suite should have cases");
        assert_eq!(last.id, "cgb-audio-samesuite-div-write-trigger-volume-10");
        assert_eq!(
            last.rom_path,
            PathBuf::from("samesuite/apu/div_write_trigger_volume_10.gb")
        );
        assert_eq!(last.timeout, Timeout::Frames(180));

        for case in &suite.cases {
            assert_eq!(case.console_model, ConsoleModel::GameBoyColor);
            assert!(
                case.rom_path.starts_with("samesuite/apu"),
                "{} should point at SameSuite APU",
                case.rom_path.display()
            );
            assert_eq!(
                case.external_rom_root_key.as_deref(),
                Some(TEST_ROM_ROOT_ENV_VAR)
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
                && case.report_model_suffix
        }));

        let suite = samesuite_cgb_extra_suite();

        assert_eq!(suite.name, "samesuite-cgb-extra");
        assert_eq!(suite.family.as_deref(), Some("samesuite"));
        assert_eq!(suite.subsystem, TestSubsystem::Apu);
        assert_eq!(suite.cases.len(), 10);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.timeout == Timeout::Frames(180)
                && case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.rom_path.starts_with("samesuite/apu")
                && matches!(
                    case.pass_condition,
                    PassCondition::FramebufferRgb555Fixture(_)
                )
        }));
        assert!(suite.cases.iter().any(|case| {
            case.id == "samesuite-cgb-apu-channel-3-channel-3-wave-ram-dac-on-rw"
                && case.rom_path
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
                && !case.report_model_suffix
        }));

        let suite = magen_cgb_extra_suite();

        assert_eq!(suite.name, "magen-cgb-extra");
        assert_eq!(suite.family.as_deref(), Some("magen"));
        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.cases.len(), 8);
        assert!(suite.cases.iter().all(|case| {
            case.console_model == ConsoleModel::GameBoyColor
                && case.startup_mode == StartupMode::SkipBoot
                && case.execution_mode == crate::ExecutionMode::Strict
                && case.timeout == Timeout::TCycles(5_000_000)
                && case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.rom_path.starts_with("magen")
                && matches!(
                    case.pass_condition,
                    PassCondition::FramebufferRgb555Fixture(_)
                )
        }));
        assert!(suite.cases.iter().any(|case| {
            case.id == "magen-cgb-bg-oam-priority"
                && case.rom_path == Path::new("magen/bg_oam_priority.gbc")
        }));
        assert!(suite_uses_extra_test_report("magen-cgb-extra"));
        assert!(!suite_uses_docboy_test_report("magen-cgb-extra"));
    }

    #[test]
    fn manifests_mark_current_gbemu_shootout_model_suffixed_rows() {
        let dmg_rows = [
            ("acid", "which.gb"),
            ("daid", "ppu_scanline_bgp.gb"),
            ("daid", "stop_instr.gb"),
            ("hacktix", "bully.gb"),
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
            assert!(case.report_model_suffix, "{family}/{rom}");
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
        assert!(cgb_which.report_model_suffix);
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
        assert!(cgb_scanline_bgp.report_model_suffix);
        assert_eq!(
            manifest_case_report_rom_display(cgb_scanline_bgp),
            "ppu_scanline_bgp.gb (GBC)"
        );

        let cgb_bully = manifests
            .iter()
            .flat_map(|manifest| &manifest.cases)
            .find(|case| {
                case.family == "hacktix"
                    && case.rom == Path::new("bully.gb")
                    && case.console_model == ConsoleModel::GameBoyColor
            })
            .expect("CGB Hacktix bully row should exist");
        assert!(cgb_bully.report_model_suffix);
        assert_eq!(
            manifest_case_report_rom_display(cgb_bully),
            "bully.gb (GBC)"
        );

        let cgb_boot_regs = manifests
            .iter()
            .flat_map(|manifest| &manifest.cases)
            .find(|case| case.family == "mooneye" && case.rom == Path::new("misc/boot_regs-cgb.gb"))
            .expect("CGB Mooneye boot_regs row should exist");
        assert!(!cgb_boot_regs.report_model_suffix);
        assert_eq!(
            manifest_case_report_rom_display(cgb_boot_regs),
            "misc/boot_regs-cgb.gb"
        );
    }

    #[test]
    fn curated_family_suite_builders_preserve_each_supported_oracle_shape() {
        let suites = curated_test_rom_family_suites();

        assert_eq!(suites.len(), 7);
        assert!(suites.iter().any(|suite| suite.name == "acid-dmg-curated"));
        assert!(suites.iter().any(|suite| suite.name == "cpp-dmg-curated"));
        assert!(suites.iter().any(|suite| suite.name == "daid-dmg-curated"));
        assert!(
            suites
                .iter()
                .any(|suite| suite.name == "hacktix-dmg-curated")
        );
        assert!(
            suites
                .iter()
                .any(|suite| suite.name == "mealybug-tearoom-dmg-curated")
        );
        assert!(
            suites
                .iter()
                .any(|suite| suite.name == "mooneye-acceptance-dmg-curated")
        );

        let acid_suite = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("acid"))
            .expect("acid suite should exist");
        let acid_info_case = acid_suite
            .cases
            .iter()
            .find(|case| case.id == "which-dmg")
            .expect("acid informational case should exist");
        assert!(matches!(
            acid_info_case.pass_condition,
            PassCondition::Informational(CaptureKind::Framebuffer)
        ));
        assert!(
            acid_info_case
                .capture_plan
                .contains(CaptureKind::Framebuffer)
        );
        assert!(acid_info_case.capture_plan.contains(CaptureKind::Snapshot));

        let acid_case = acid_suite
            .cases
            .iter()
            .find(|case| case.id == "dmg-acid2")
            .expect("acid framebuffer fixture case should exist");
        assert!(matches!(
            acid_case.pass_condition,
            PassCondition::FramebufferFixture(_)
        ));
        assert!(acid_case.capture_plan.contains(CaptureKind::Framebuffer));
        assert!(acid_case.capture_plan.contains(CaptureKind::Snapshot));

        let cpp_suite = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("cpp"))
            .expect("cpp suite should exist");
        assert_eq!(cpp_suite.cases.len(), 3);
        assert!(
            cpp_suite
                .cases
                .iter()
                .all(|case| matches!(case.pass_condition, PassCondition::FramebufferFixture(_)))
        );

        let halt_bug_case = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("blargg"))
            .and_then(|suite| suite.cases.iter().find(|case| case.id == "blargg-halt-bug"))
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
                    .find(|case| case.id == "mealybug-m3-bgp-change-sprites")
            })
            .expect("mealybug custom-boot case should exist");
        assert_eq!(mealybug_case.startup_mode, StartupMode::CustomBoot);
        assert!(mealybug_case.startup_memory_writes.is_empty());

        let hacktix_case = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("hacktix"))
            .and_then(|suite| suite.cases.iter().find(|case| case.id == "hacktix-bully"))
            .expect("hacktix custom-boot case should exist");
        assert_eq!(hacktix_case.startup_mode, StartupMode::CustomBoot);
        assert!(hacktix_case.startup_memory_writes.is_empty());

        let strikethrough_case = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("hacktix"))
            .and_then(|suite| {
                suite
                    .cases
                    .iter()
                    .find(|case| case.id == "hacktix-strikethrough")
            })
            .expect("hacktix framebuffer case should exist");
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
                "ax6".to_string(),
                "blargg".to_string(),
                "cpp".to_string(),
                "daid".to_string(),
                "docboy-cgb".to_string(),
                "docboy-cgb-dmg".to_string(),
                "docboy-cgb-dmg-ext".to_string(),
                "docboy-dmg".to_string(),
                "gbmicrotest".to_string(),
                "hacktix".to_string(),
                "little-things-gb".to_string(),
                "magen".to_string(),
                "mealybug-tearoom-tests".to_string(),
                "mooneye".to_string(),
                "samesuite".to_string(),
            ]
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
    fn materialize_curated_store_copies_roms_and_replaces_existing_family_dirs() {
        let workspace_root = unique_temp_dir("materialize");
        let gbemu_shootout_root = workspace_root.join("gbemu-shootout");
        write_fake_gbemu_shootout_tree(&gbemu_shootout_root);

        let stale_family_root = test_rom_store_root(&workspace_root).join("acid");
        fs::create_dir_all(&stale_family_root).expect("stale family root should be creatable");
        fs::write(stale_family_root.join("stale.txt"), "old").expect("stale file should write");

        materialize_curated_test_rom_store(&workspace_root, &gbemu_shootout_root)
            .expect("curated test ROM store should materialize");

        assert!(!stale_family_root.join("stale.txt").exists());
        for manifest in curated_test_rom_manifests() {
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
                test_rom_store_root(&workspace_root).join("mooneye/misc/boot_regs-cgb.gb")
            )
            .expect("CGB smoke ROM should be materialized from the cgb-smoke manifest"),
            "mooneye:misc/boot_regs-cgb.gb"
        );
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
        assert!(error.contains("testroms/acid/"));

        fs::remove_dir_all(workspace_root).expect("workspace root should be removable");
    }

    #[test]
    fn materialize_curated_store_can_limit_selected_families_without_touching_others() {
        let workspace_root = unique_temp_dir("materialize-selected");
        let gbemu_shootout_root = workspace_root.join("gbemu-shootout");
        write_fake_gbemu_shootout_tree(&gbemu_shootout_root);

        let store_root = test_rom_store_root(&workspace_root);
        let acid_root = store_root.join("acid");
        let mooneye_root = store_root.join("mooneye");
        fs::create_dir_all(&acid_root).expect("acid family root should be creatable");
        fs::create_dir_all(&mooneye_root).expect("mooneye family root should be creatable");
        fs::write(acid_root.join("stale.txt"), "replace").expect("acid stale file should write");
        fs::write(mooneye_root.join("keep.txt"), "keep").expect("mooneye marker should write");

        materialize_curated_test_rom_families(
            &workspace_root,
            &gbemu_shootout_root,
            &["acid".to_string()],
        )
        .expect("selected curated families should materialize");

        assert!(!acid_root.join("stale.txt").exists());
        assert!(acid_root.join("which.gb").exists());
        assert!(acid_root.join("dmg-acid2.gb").exists());
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
        fs::create_dir_all(test_rom_store_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let first_report = RomSuiteReport {
            suite_name: "blargg-dmg-curated".to_string(),
            family: Some("blargg".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
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
            suite_name: "blargg-dmg-curated".to_string(),
            family: Some("blargg".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![RomCaseReport {
                case_id: "blargg-halt-bug".to_string(),
                rom_path: PathBuf::from("blargg/halt_bug.gb"),
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

        let suite_status_path = test_rom_store_root(&workspace_root)
            .join(TEST_ROM_STATUS_DIR_NAME)
            .join("blargg-dmg-curated.toml");
        let suite_status =
            fs::read_to_string(&suite_status_path).expect("suite status should be readable");
        assert!(suite_status.contains("cpu_instrs/01-special.gb"));
        assert!(suite_status.contains("halt_bug.gb"));

        let rendered_report =
            fs::read_to_string(report_path).expect("markdown report should be readable");
        assert!(rendered_report.starts_with("# Test Report (1/2)\n"));
        assert!(rendered_report.contains(&format!(
            "| blargg | cpu_instrs/01-special.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(rendered_report.contains(&format!(
            "| blargg | halt_bug.gb | {REPORT_STATUS_FAIL_EMOJI} |"
        )));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_updates_existing_case_status_and_ignores_non_toml_entries() {
        let workspace_root = unique_temp_dir("report-update");
        let status_root = test_rom_store_root(&workspace_root).join(TEST_ROM_STATUS_DIR_NAME);
        fs::create_dir_all(&status_root).expect("status root should be creatable");
        fs::write(status_root.join("README.txt"), "ignore me")
            .expect("non-toml status marker should be writable");

        let failing_report = RomSuiteReport {
            suite_name: "blargg-dmg-curated".to_string(),
            family: Some("blargg".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![report_case(
                "blargg-halt-bug",
                "blargg/halt_bug.gb",
                RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded),
            )],
        };
        update_curated_test_report(&workspace_root, &failing_report)
            .expect("failing partial report should write");

        let passing_report = RomSuiteReport {
            suite_name: "blargg-dmg-curated".to_string(),
            family: Some("blargg".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![report_case(
                "blargg-halt-bug",
                "blargg/halt_bug.gb",
                RomCaseOutcome::Passed,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &passing_report)
            .expect("passing partial report should write")
            .expect("curated suite should emit a report path");

        let suite_status_path = status_root.join("blargg-dmg-curated.toml");
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
    fn curated_test_report_omits_status_only_mooneye_curated_rows_from_markdown() {
        let workspace_root = unique_temp_dir("report-status-only-mooneye");
        fs::create_dir_all(test_rom_store_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let manual_report = RomSuiteReport {
            suite_name: "mooneye-dmg-acceptance-manual".to_string(),
            family: Some("mooneye".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![
                report_case(
                    "mooneye-boot-hwio-dmgabcmgb",
                    "mooneye/acceptance/boot_hwio-dmgABCmgb.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "mooneye-ppu-lcdon-timing-gs",
                    "mooneye/acceptance/ppu/lcdon_timing-GS.gb",
                    RomCaseOutcome::Passed,
                ),
            ],
        };
        update_curated_test_report(&workspace_root, &manual_report)
            .expect("Mooneye manual report should write");

        let status_only_report = RomSuiteReport {
            suite_name: "mooneye-acceptance-dmg-curated".to_string(),
            family: Some("mooneye".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![
                report_case(
                    "mooneye-boot-hwio-dmgabcmgb",
                    "mooneye/acceptance/boot_hwio-dmgABCmgb.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "mooneye-emulator-only-mbc1-bits-bank1",
                    "mooneye/emulator-only/mbc1/bits_bank1.gb",
                    RomCaseOutcome::Passed,
                ),
            ],
        };
        assert_eq!(
            update_curated_test_report(&workspace_root, &status_only_report)
                .expect("Mooneye status-only report should write"),
            None
        );

        let status_root = test_rom_store_root(&workspace_root).join(TEST_ROM_STATUS_DIR_NAME);
        let status_only_status =
            fs::read_to_string(status_root.join("mooneye-acceptance-dmg-curated.toml"))
                .expect("status-only Mooneye status should be readable");
        assert!(status_only_status.contains("acceptance/boot_hwio-dmgABCmgb.gb"));
        assert!(status_only_status.contains("emulator-only/mbc1/bits_bank1.gb"));

        let rendered_report = fs::read_to_string(
            test_rom_store_root(&workspace_root).join(TEST_ROM_REPORT_FILE_NAME),
        )
        .expect("markdown report should be readable");
        assert!(rendered_report.starts_with("# Test Report (2/2)\n"));
        assert_eq!(
            rendered_report
                .matches("| mooneye | acceptance/boot_hwio-dmgABCmgb.gb |")
                .count(),
            1
        );
        assert!(rendered_report.contains(&format!(
            "| mooneye | acceptance/ppu/lcdon_timing-GS.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(!rendered_report.contains("emulator-only/mbc1/bits_bank1.gb"));

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_persists_and_renders_informational_statuses() {
        let workspace_root = unique_temp_dir("report-info");
        fs::create_dir_all(test_rom_store_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let report = RomSuiteReport {
            suite_name: "acid-dmg-curated".to_string(),
            family: Some("acid".to_string()),
            subsystem: TestSubsystem::Ppu,
            cases: vec![report_case(
                "which-dmg",
                "acid/which.gb",
                RomCaseOutcome::Informational,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &report)
            .expect("informational report should write")
            .expect("curated suite should emit a report path");

        let suite_status_path = test_rom_store_root(&workspace_root)
            .join(TEST_ROM_STATUS_DIR_NAME)
            .join("acid-dmg-curated.toml");
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
    fn cgb_smoke_report_uses_upstream_case_families_and_shootout_suffixes() {
        let workspace_root = unique_temp_dir("report-cgb-smoke");
        fs::create_dir_all(test_rom_store_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let report = RomSuiteReport {
            suite_name: "cgb-smoke".to_string(),
            family: Some("cgb-smoke".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![
                report_case(
                    "cgb-smoke-boot-regs-cgb",
                    "mooneye/misc/boot_regs-cgb.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "cgb-smoke-which-gbc",
                    "acid/which.gb",
                    RomCaseOutcome::Informational,
                ),
            ],
        };
        let report_path = update_curated_test_report(&workspace_root, &report)
            .expect("CGB smoke report should write")
            .expect("curated suite should emit a report path");

        let suite_status_path = test_rom_store_root(&workspace_root)
            .join(TEST_ROM_STATUS_DIR_NAME)
            .join("cgb-smoke.toml");
        let suite_status =
            fs::read_to_string(&suite_status_path).expect("suite status should be readable");
        assert!(suite_status.contains("family = \"acid\""));
        assert!(suite_status.contains("rom = \"which.gb (GBC)\""));
        assert!(suite_status.contains("family = \"mooneye\""));
        assert!(suite_status.contains("rom = \"misc/boot_regs-cgb.gb\""));

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
    fn curated_test_report_routes_extra_suites_to_extra_markdown_file() {
        let workspace_root = unique_temp_dir("report-cgb-extra");
        fs::create_dir_all(test_rom_store_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let cgb_smoke_report = RomSuiteReport {
            suite_name: "cgb-smoke".to_string(),
            family: Some("cgb-smoke".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![report_case(
                "cgb-smoke-boot-regs-cgb",
                "mooneye/misc/boot_regs-cgb.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &cgb_smoke_report)
            .expect("CGB smoke report should write");

        let cgb_rtc_report = RomSuiteReport {
            suite_name: "cgb-rtc".to_string(),
            family: Some("cgb-rtc".to_string()),
            subsystem: TestSubsystem::Cartridge,
            cases: vec![report_case(
                "cgb-rtc-rtc3test-1",
                "ax6/rtc3test-1.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &cgb_rtc_report)
            .expect("CGB RTC report should write");

        let cgb_audio_samesuite_report = RomSuiteReport {
            suite_name: "cgb-audio-samesuite".to_string(),
            family: Some("cgb-audio-samesuite".to_string()),
            subsystem: TestSubsystem::Apu,
            cases: vec![
                report_case(
                    "cgb-audio-samesuite-div-write-trigger",
                    "samesuite/apu/div_write_trigger.gb",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "cgb-audio-samesuite-div-write-trigger-10",
                    "samesuite/apu/div_write_trigger_10.gb",
                    RomCaseOutcome::Passed,
                ),
            ],
        };
        update_curated_test_report(&workspace_root, &cgb_audio_samesuite_report)
            .expect("CGB SameSuite report should write");

        let cgb_boot_hwio_report = RomSuiteReport {
            suite_name: "cgb-boot-hwio".to_string(),
            family: Some("cgb-boot-hwio".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![report_case(
                "cgb-boot-hwio-boot-hwio-c",
                "mooneye/misc/boot_hwio-C.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &cgb_boot_hwio_report)
            .expect("CGB boot HWIO report should write")
            .expect("extra curated suite should emit a report path");

        let ax6_report = RomSuiteReport {
            suite_name: "ax6-dmg-extra".to_string(),
            family: Some("ax6".to_string()),
            subsystem: TestSubsystem::Cartridge,
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
            subsystem: TestSubsystem::Apu,
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
            subsystem: TestSubsystem::Apu,
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
            subsystem: TestSubsystem::CrossSubsystem,
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
            subsystem: TestSubsystem::CrossSubsystem,
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
            subsystem: TestSubsystem::CrossSubsystem,
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

        let standard_report = fs::read_to_string(
            test_rom_store_root(&workspace_root).join(TEST_ROM_REPORT_FILE_NAME),
        )
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
        assert!(!standard_report.contains("div_write_trigger.gb (DMG)"));
        assert!(!standard_report.contains("div_write_trigger_10.gb (DMG)"));
        assert!(!standard_report.contains("ei_delay_halt.gb"));
        assert!(!standard_report.contains("rtc3test-1.gb (DMG)"));
        assert!(!standard_report.contains("whichboot.gb"));

        let extra_report =
            fs::read_to_string(report_path).expect("extra report should be readable");
        assert!(extra_report.starts_with("# Test Report (12/12)\n"));
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
        let store_root = test_rom_store_root(&workspace_root);
        fs::create_dir_all(&store_root).expect("test rom store root should be creatable");
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
            suite_name: "docboy-cgb-extra".to_string(),
            family: Some("docboy-cgb".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![report_case(
                "docboy-cgb-docboy-boot-boot-bg-palettes",
                "docboy/cgb/boot/boot_bg_palettes.gbc",
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
        assert!(!store_root.join(TEST_ROM_REPORT_FILE_NAME).exists());
        assert!(!store_root.join(TEST_ROM_EXTRA_REPORT_FILE_NAME).exists());

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
    }

    #[test]
    fn curated_test_report_routes_docboy_suites_to_docboy_markdown_file() {
        let workspace_root = unique_temp_dir("report-docboy");
        fs::create_dir_all(test_rom_store_root(&workspace_root))
            .expect("test rom store root should be creatable");

        let cgb_smoke_report = RomSuiteReport {
            suite_name: "cgb-smoke".to_string(),
            family: Some("cgb-smoke".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![report_case(
                "cgb-smoke-boot-regs-cgb",
                "mooneye/misc/boot_regs-cgb.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &cgb_smoke_report)
            .expect("CGB smoke report should write");

        let extra_report = RomSuiteReport {
            suite_name: "gbmicrotest-dmg-extra".to_string(),
            family: Some("gbmicrotest".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![report_case(
                "gbmicrotest-boot-poweron-bgp-000",
                "gbmicrotest/boot/poweron_bgp_000.gb",
                RomCaseOutcome::Passed,
            )],
        };
        update_curated_test_report(&workspace_root, &extra_report)
            .expect("extra report should write");

        let docboy_report = RomSuiteReport {
            suite_name: "docboy-cgb-extra".to_string(),
            family: Some("docboy-cgb".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
            cases: vec![
                report_case(
                    "docboy-cgb-docboy-boot-boot-bg-palettes",
                    "docboy/cgb/boot/boot_bg_palettes.gbc",
                    RomCaseOutcome::Passed,
                ),
                report_case(
                    "docboy-cgb-docboy-boot-boot-bg-palettes-fail",
                    "docboy/cgb/boot/boot_bg_palettes.gbc",
                    RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded),
                ),
            ],
        };
        let report_path = update_curated_test_report(&workspace_root, &docboy_report)
            .expect("DocBoy report should write")
            .expect("DocBoy suite should emit a report path");
        assert_eq!(
            report_path,
            test_rom_store_root(&workspace_root).join(TEST_ROM_DOCBOY_REPORT_FILE_NAME)
        );

        let standard_report = fs::read_to_string(
            test_rom_store_root(&workspace_root).join(TEST_ROM_REPORT_FILE_NAME),
        )
        .expect("standard report should be readable");
        assert!(standard_report.contains("boot_regs-cgb.gb"));
        assert!(!standard_report.contains("boot_bg_palettes.gbc"));
        assert!(!standard_report.contains("poweron_bgp_000.gb"));

        let rendered_extra = fs::read_to_string(
            test_rom_store_root(&workspace_root).join(TEST_ROM_EXTRA_REPORT_FILE_NAME),
        )
        .expect("extra report should be readable");
        assert!(rendered_extra.contains("poweron_bgp_000.gb"));
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
    fn curated_test_report_prunes_extra_model_rows_from_promoted_suite_status() {
        let workspace_root = unique_temp_dir("report-prune-extra-model-rows");
        let status_root = test_rom_store_root(&workspace_root).join(TEST_ROM_STATUS_DIR_NAME);
        fs::create_dir_all(&status_root).expect("status root should be creatable");
        fs::write(
            status_root.join("cgb-rtc.toml"),
            r#"version = 1
suite_name = "cgb-rtc"
family = "cgb-rtc"

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
        .expect("stale CGB RTC status should be writable");

        let cgb_rtc_report = RomSuiteReport {
            suite_name: "cgb-rtc".to_string(),
            family: Some("cgb-rtc".to_string()),
            subsystem: TestSubsystem::Cartridge,
            cases: vec![report_case(
                "cgb-rtc-rtc3test-1",
                "ax6/rtc3test-1.gb",
                RomCaseOutcome::Passed,
            )],
        };
        let report_path = update_curated_test_report(&workspace_root, &cgb_rtc_report)
            .expect("CGB RTC report should write")
            .expect("curated suite should emit a report path");

        let suite_status = fs::read_to_string(status_root.join("cgb-rtc.toml"))
            .expect("CGB RTC status should be readable");
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
            version: 1,
            suite_name: "cgb-smoke".to_string(),
            family: "cgb-smoke".to_string(),
            cases: vec![PersistedCaseStatus {
                family: None,
                rom: "mooneye/misc/boot_regs-cgb.gb".to_string(),
                status: "PASS".to_string(),
            }],
        }]);

        assert!(rendered.starts_with("# Test Report (0/0)\n"));
        assert!(!rendered.contains(&format!(
            "| cgb-smoke | misc/boot_regs-cgb.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
        assert!(!rendered.contains(&format!(
            "| mooneye | misc/boot_regs-cgb.gb | {REPORT_STATUS_PASS_EMOJI} |"
        )));
    }

    #[test]
    fn render_markdown_report_orders_mixed_rows_by_shootout_source_order() {
        let rendered = render_markdown_report(&[
            PersistedSuiteStatus {
                version: 1,
                suite_name: "acid-dmg-curated".to_string(),
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
                ],
            },
            PersistedSuiteStatus {
                version: 1,
                suite_name: "mooneye-acceptance-dmg-curated".to_string(),
                family: "mooneye".to_string(),
                cases: vec![PersistedCaseStatus {
                    family: None,
                    rom: "manual-only/sprite_priority.gb".to_string(),
                    status: "PASS".to_string(),
                }],
            },
            PersistedSuiteStatus {
                version: 1,
                suite_name: "cgb-smoke".to_string(),
                family: "cgb-smoke".to_string(),
                cases: vec![
                    PersistedCaseStatus {
                        family: Some("mooneye".to_string()),
                        rom: "misc/boot_regs-cgb.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                    PersistedCaseStatus {
                        family: Some("acid".to_string()),
                        rom: "which.gb (GBC)".to_string(),
                        status: "INFO".to_string(),
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
            .expect("Acid dmg-acid2 row should exist");
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
    fn curated_test_report_returns_none_for_non_curated_suites() {
        let workspace_root = unique_temp_dir("report-none");
        let report = RomSuiteReport {
            suite_name: "phase-2-cpu-timing".to_string(),
            family: None,
            subsystem: TestSubsystem::Cpu,
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
            suite_name: "acid-dmg-curated".to_string(),
            family: Some("acid".to_string()),
            subsystem: TestSubsystem::Ppu,
            cases: vec![report_case(
                "which-dmg",
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
        let status_root = test_rom_store_root(&workspace_root).join(TEST_ROM_STATUS_DIR_NAME);
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
        let report_path = test_rom_store_root(&workspace_root).join(TEST_ROM_REPORT_FILE_NAME);
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
                version: 1,
                suite_name: "mooneye-acceptance-dmg-curated".to_string(),
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
                version: 1,
                suite_name: "acid-dmg-curated".to_string(),
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
        assert!(!rendered.contains("| hacktix | - | - |"));
        assert!(!rendered.contains("| cpp | - | - |"));
        assert!(!rendered.contains("| mealybug-tearoom-tests | - | - |"));
        assert!(!rendered.contains("| little-things-gb | - | - |"));
    }

    #[test]
    fn render_markdown_report_keeps_unknown_families_when_they_are_present() {
        let rendered = render_markdown_report(&[PersistedSuiteStatus {
            version: 1,
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
            suite_name: "acid-dmg-curated".to_string(),
            family: Some("acid".to_string()),
            subsystem: TestSubsystem::Ppu,
            cases: vec![
                report_case("acid-which", "acid/which.gb", RomCaseOutcome::Informational),
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
            suite_name: "blargg-dmg-curated".to_string(),
            family: Some("blargg".to_string()),
            subsystem: TestSubsystem::CrossSubsystem,
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

        sort_persisted_case_statuses("acid-dmg-curated", "acid", &mut case_statuses);

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
        fs::write(&path, "version = [").expect("invalid status file should be writable");

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
    #[should_panic(expected = "unsupported subsystem")]
    fn parse_manifest_subsystem_rejects_unknown_values() {
        let _ = parse_manifest_subsystem("test-manifest.toml", "Cpu");
    }

    #[test]
    #[should_panic(expected = "unsupported console model")]
    fn parse_manifest_console_model_rejects_unknown_values() {
        let _ = parse_manifest_console_model("test-manifest.toml", "test-case", "sgb");
    }

    #[test]
    #[should_panic(expected = "missing family")]
    fn parse_manifest_case_rejects_familyless_mixed_cases() {
        let _ = parse_manifest_case(
            "test-manifest.toml",
            None,
            CuratedTestRomCaseFile {
                family: None,
                id: "familyless".to_string(),
                rom: PathBuf::from("familyless.gb"),
                source_id: None,
                source_path: None,
                report_model_suffix: None,
                report_label: None,
                timeout_frames: Some(1),
                timeout_tcycles: None,
                oracle: "info-framebuffer".to_string(),
                expected: None,
                fixture: None,
                fixtures: None,
                check_interval_tcycles: None,
                check_at_tcycles: None,
                memory: Vec::new(),
                stimuli: Vec::new(),
                console: None,
                revision: None,
                startup: None,
                execution_mode: None,
                stop_condition: None,
                disabled: false,
                comment: None,
            },
        );
    }

    #[test]
    fn parse_manifest_case_preserves_disabled_case_comment() {
        let case = parse_manifest_case(
            "test-manifest.toml",
            Some("docboy-dmg"),
            CuratedTestRomCaseFile {
                family: None,
                id: "disabled-with-comment".to_string(),
                rom: PathBuf::from("disabled.gb"),
                source_id: None,
                source_path: None,
                report_model_suffix: None,
                report_label: None,
                timeout_frames: Some(1),
                timeout_tcycles: None,
                oracle: "info-framebuffer".to_string(),
                expected: None,
                fixture: None,
                fixtures: None,
                check_interval_tcycles: None,
                check_at_tcycles: None,
                memory: Vec::new(),
                stimuli: Vec::new(),
                console: Some("dmg".to_string()),
                revision: None,
                startup: None,
                execution_mode: None,
                stop_condition: None,
                disabled: true,
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
            CuratedTestRomCaseFile {
                family: None,
                id: "disabled-without-comment".to_string(),
                rom: PathBuf::from("disabled.gb"),
                source_id: None,
                source_path: None,
                report_model_suffix: None,
                report_label: None,
                timeout_frames: Some(1),
                timeout_tcycles: None,
                oracle: "info-framebuffer".to_string(),
                expected: None,
                fixture: None,
                fixtures: None,
                check_interval_tcycles: None,
                check_at_tcycles: None,
                memory: Vec::new(),
                stimuli: Vec::new(),
                console: Some("dmg".to_string()),
                revision: None,
                startup: None,
                execution_mode: None,
                stop_condition: None,
                disabled: true,
                comment: Some("   ".to_string()),
            },
        );
    }

    #[test]
    #[should_panic(expected = "unsupported oracle")]
    fn manifest_case_to_rom_test_case_rejects_unknown_oracles() {
        let _ = manifest_case_to_rom_test_case(CuratedTestRomCase {
            family: "blargg".to_string(),
            id: "bad-oracle".to_string(),
            rom: PathBuf::from("bad.gb"),
            source_id: GBEMU_SHOOTOUT_SOURCE_ID.to_string(),
            source_path: PathBuf::from("testroms/blargg/bad.gb"),
            report_model_suffix: false,
            report_label: None,
            timeout: Timeout::Frames(1),
            oracle: "unknown".to_string(),
            expected: None,
            fixture: None,
            fixtures: None,
            check_interval_tcycles: None,
            check_at_tcycles: None,
            memory: Vec::new(),
            stimuli: Vec::new(),
            console_model: ConsoleModel::GameBoy,
            revision: HardwareRevision::DmgCpuC,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: None,
            stop_condition: None,
            disabled: false,
            comment: None,
        });
    }

    #[test]
    fn report_file_name_stays_stable() {
        assert_eq!(TEST_ROM_REPORT_FILE_NAME, "test-report.md");
        assert_eq!(TEST_ROM_EXTRA_REPORT_FILE_NAME, "test-report-extra.md");
        assert_eq!(TEST_ROM_DOCBOY_REPORT_FILE_NAME, "test-report-docboy.md");
        assert!(suite_uses_extra_test_report("ax6-dmg-extra"));
        assert!(suite_uses_extra_test_report("cgb-boot-hwio"));
        assert!(suite_uses_extra_test_report("samesuite-dmg-extra"));
        assert!(suite_uses_extra_test_report("little-things-gb-dmg-extra"));
        assert!(suite_uses_docboy_test_report("docboy-dmg-extra"));
        assert!(suite_uses_docboy_test_report("docboy-cgb-extra"));
        assert!(suite_uses_docboy_test_report("docboy-cgb-dmg-extra"));
        assert!(suite_uses_docboy_test_report("docboy-cgb-dmg-ext-extra"));
        assert!(!suite_uses_extra_test_report("docboy-dmg-extra"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-extra"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-dmg-extra"));
        assert!(!suite_uses_extra_test_report("docboy-cgb-dmg-ext-extra"));
        assert!(!suite_uses_extra_test_report("cgb-smoke"));
    }
}
