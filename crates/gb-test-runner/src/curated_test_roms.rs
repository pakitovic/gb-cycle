use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use gb_core::{ConsoleModel, StartupMode, TimerStartupState};
use serde::{Deserialize, Serialize};

use crate::{
    CaptureKind, CapturePlan, ExecutionMode, FailureArtifactPolicy, MemoryTextOutputSpec,
    PassCondition, RomSuite, RomSuiteReport, RomTestCase, StartupMemoryWrite, TestSubsystem,
    Timeout,
};

pub const TEST_ROM_STORE_DIR: &str = ".roms/test";
pub const TEST_ROM_ROOT_ENV_VAR: &str = "GB_CYCLE_TEST_ROM_ROOT";
pub const TEST_ROM_REPORT_FILE_NAME: &str = "test-report.md";

const TEST_ROM_STATUS_DIR_NAME: &str = ".status";
const CURATED_TEST_ROM_MANIFEST_VERSION: u32 = 1;
const CURATED_SOURCE_MANIFEST_VERSION: u32 = 1;
const CURATED_TEST_ROM_REPORT_VERSION: u32 = 1;
const GBEMU_SHOOTOUT_TESTROMS_DIR: &str = "testroms";
const REPORT_STATUS_PASS_EMOJI: &str = "✅";
const REPORT_STATUS_FAIL_EMOJI: &str = "❌";
const REPORT_STATUS_INFO_EMOJI: &str = "ℹ️";
const CURATED_TEST_ROM_REPORT_FAMILY_ORDER: [&str; 9] = [
    "acid",
    "blargg",
    "daid",
    "ax6",
    "mooneye",
    "samesuite",
    "hacktix",
    "cpp",
    "mealybug-tearoom-tests",
];
const DMG_BOOT_TRADEMARK_TILE_VRAM_START: u16 = 0x8190;
const DMG_BOOT_TRADEMARK_TILE_BYTES: [u8; 16] = [
    0x3C, 0x00, 0x42, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0x42, 0x00, 0x3C, 0x00,
];
const DMG_BOOT_LOGO_TILE_VRAM_START: u16 = 0x8010;
const DMG_BOOT_LOGO_MAP_VRAM_START: u16 = 0x9904;
const DMG_BOOT_LOGO_TILE_BYTES: [u8; 200] = [
    0xF0, 0xF0, 0xFC, 0xFC, 0xFC, 0xFC, 0xF3, 0xF3, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0xF0, 0xF0, 0xF0, 0xF0, 0x00, 0x00, 0xF3, 0xF3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xCF, 0xCF,
    0x00, 0x00, 0x0F, 0x0F, 0x3F, 0x3F, 0x0F, 0x0F, 0x00, 0x00, 0x00, 0x00, 0xC0, 0xC0, 0x0F, 0x0F,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF0, 0xF0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xF3, 0xF3,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0xC0, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0xFF, 0xFF,
    0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC3, 0xC3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFC, 0xFC,
    0xF3, 0xF3, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0x3C, 0x3C, 0xFC, 0xFC, 0xFC, 0xFC, 0x3C, 0x3C,
    0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3,
    0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0x3C, 0x3C, 0x3F, 0x3F, 0x3C, 0x3C, 0x0F, 0x0F,
    0x3C, 0x3C, 0xFC, 0xFC, 0x00, 0x00, 0xFC, 0xFC, 0xFC, 0xFC, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0, 0xF0,
    0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF3, 0xF0, 0xF0, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xC3, 0xFF, 0xFF,
    0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xCF, 0xC3, 0xC3, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0xFC, 0xFC,
    0x3C, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C,
];
const DMG_BOOT_LOGO_MAP_BYTES: [u8; 44] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x19, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
];
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
    report_model_suffix: Option<bool>,
    timeout_frames: u32,
    oracle: String,
    expected: Option<String>,
    fixture: Option<PathBuf>,
    fixtures: Option<Vec<PathBuf>>,
    console: Option<String>,
    startup: Option<String>,
    execution_mode: Option<String>,
    startup_timer_profile: Option<String>,
    startup_memory_profile: Option<String>,
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
    report_model_suffix: bool,
    timeout_frames: u32,
    oracle: String,
    expected: Option<String>,
    fixture: Option<PathBuf>,
    fixtures: Option<Vec<PathBuf>>,
    console_model: ConsoleModel,
    startup_mode: StartupMode,
    execution_mode: Option<String>,
    startup_timer_profile: Option<String>,
    startup_memory_profile: Option<String>,
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
    #[serde(default, rename = "required_file")]
    required_files: Vec<CuratedRequiredFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedRequiredFile {
    path: PathBuf,
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

pub fn acid_dmg_curated_suite() -> RomSuite {
    manifest_suite("acid")
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

pub fn materialize_curated_test_rom_store(
    workspace_root: &Path,
    gbemu_shootout_root: &Path,
) -> Result<(), String> {
    materialize_curated_test_rom_store_filtered(workspace_root, gbemu_shootout_root, None)
}

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
    gbemu_shootout_root: &Path,
    selected_families: Option<&BTreeSet<&str>>,
) -> Result<(), String> {
    let store_root = test_rom_store_root(workspace_root);
    fs::create_dir_all(&store_root).map_err(|error| {
        format!(
            "failed to create curated test ROM store {}: {error}",
            store_root.display()
        )
    })?;

    let selected_roms_by_family = curated_test_roms_by_family(selected_families);
    let mut materialized_families = BTreeSet::new();
    for (family, roms) in selected_roms_by_family {
        materialized_families.insert(family.clone());
        let family_root = store_root.join(&family);
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

        for rom in roms {
            copy_curated_rom(gbemu_shootout_root, &family, &rom, &family_root.join(&rom))?;
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

fn curated_test_roms_by_family(
    selected_families: Option<&BTreeSet<&str>>,
) -> BTreeMap<String, BTreeSet<PathBuf>> {
    let mut roms_by_family = BTreeMap::<String, BTreeSet<PathBuf>>::new();
    for manifest in curated_test_rom_manifests() {
        for case in manifest.cases {
            if let Some(selected_families) = selected_families
                && !selected_families.contains(case.family.as_str())
            {
                continue;
            }
            roms_by_family
                .entry(case.family)
                .or_default()
                .insert(case.rom);
        }
    }
    roms_by_family
}

fn copy_curated_rom(
    gbemu_shootout_root: &Path,
    family: &str,
    rom: &Path,
    target_path: &Path,
) -> Result<(), String> {
    let source_path = gbemu_shootout_root
        .join(GBEMU_SHOOTOUT_TESTROMS_DIR)
        .join(family)
        .join(rom);
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

    let suite_status_path = status_root.join(format!("{}.toml", report.suite_name));
    let mut merged_case_statuses = load_persisted_suite_status(&suite_status_path)?
        .filter(|persisted| {
            persisted.suite_name == report.suite_name && persisted.family == *family
        })
        .map_or_else(Vec::new, |persisted| persisted.cases);

    for case in &report.cases {
        let metadata = report_case_metadata(&report.suite_name, family, case);
        let legacy_rom = report_rom_display(family, &case.rom_path);
        let legacy_full_rom = case.rom_path.to_string_lossy();
        let status = case.outcome.report_status().to_string();

        merged_case_statuses.retain(|entry| {
            let entry_family = entry.family.as_deref().unwrap_or(family);
            !(entry_family == metadata.family && entry.rom == metadata.rom
                || entry.family.is_none() && entry.rom == legacy_rom
                || entry.family.is_none() && entry.rom == legacy_full_rom.as_ref())
        });
        merged_case_statuses.push(PersistedCaseStatus {
            family: (metadata.family != *family).then_some(metadata.family),
            rom: metadata.rom,
            status,
        });
    }
    sort_persisted_case_statuses(family, &mut merged_case_statuses);

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
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let text = fs::read_to_string(entry.path()).map_err(|error| {
            format!(
                "failed to read curated test ROM status {}: {error}",
                entry.path().display()
            )
        })?;
        let persisted: PersistedSuiteStatus = toml::from_str(&text).map_err(|error| {
            format!(
                "failed to parse curated test ROM status {}: {error}",
                entry.path().display()
            )
        })?;
        suites.push(normalize_persisted_suite_status(persisted));
    }

    suites.sort_by(compare_report_suites);

    let report_path = store_root.join(TEST_ROM_REPORT_FILE_NAME);
    fs::write(&report_path, render_markdown_report(&suites)).map_err(|error| {
        format!(
            "failed to write curated test ROM report {}: {error}",
            report_path.display()
        )
    })?;

    Ok(Some(report_path))
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
    for case in &mut persisted.cases {
        let family = case.family.as_deref().unwrap_or(&persisted.family);
        if let Some(normalized_rom) = manifest_report_rom_for_persisted_case(family, &case.rom) {
            case.rom = normalized_rom;
        }
    }
    sort_persisted_case_statuses(&persisted.family, &mut persisted.cases);
    persisted
}

fn manifest_report_rom_for_persisted_case(family: &str, rom: &str) -> Option<String> {
    curated_test_rom_manifests()
        .into_iter()
        .flat_map(|manifest| manifest.cases)
        .filter(|case| case.family == family)
        .find(|case| {
            let rom_path = PathBuf::from(&case.family).join(&case.rom);
            report_rom_display(&case.family, &rom_path) == rom
                || manifest_case_report_rom_display(case) == rom
                || rom_path.to_string_lossy() == rom
        })
        .map(|case| manifest_case_report_rom_display(&case))
}

fn manifest_case_order(family: &str, rom: &str) -> Option<ReportCaseOrder> {
    for (case_manifest_order, case) in curated_test_rom_manifests()
        .into_iter()
        .flat_map(|manifest| manifest.cases)
        .filter(|case| case.family == family)
        .enumerate()
    {
        if report_rom_display(&case.family, &PathBuf::from(&case.family).join(&case.rom)) == rom
            || manifest_case_report_rom_display(&case) == rom
        {
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

fn sort_persisted_case_statuses(family: &str, case_statuses: &mut [PersistedCaseStatus]) {
    case_statuses.sort_by(|left, right| {
        let left_family = left.family.as_deref().unwrap_or(family);
        let right_family = right.family.as_deref().unwrap_or(family);
        let left_rank = report_family_rank(left_family);
        let right_rank = report_family_rank(right_family);
        let left_order = manifest_case_order(left_family, &left.rom);
        let right_order = manifest_case_order(right_family, &right.rom);
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
    });
}

fn manifest_suite(family: &str) -> RomSuite {
    let manifest = curated_test_rom_manifests()
        .into_iter()
        .find(|manifest| manifest.suite_family.as_deref() == Some(family))
        .unwrap_or_else(|| panic!("missing curated test ROM manifest for family {family}"));

    let mut suite = RomSuite::new(manifest.suite_name, manifest.subsystem).with_family(family);
    for case in manifest.cases {
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

pub fn cgb_dma_suite() -> RomSuite {
    manifest_suite_by_name("cgb-dma")
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
        suite.push_case(manifest_case_to_rom_test_case(case));
    }
    suite
}

fn curated_test_rom_manifests() -> Vec<CuratedTestRomManifest> {
    curated_test_rom_manifest_texts()
        .into_iter()
        .map(|(source_path, source_text)| parse_manifest(source_path, source_text))
        .collect()
}

fn curated_test_rom_manifest_texts() -> [(&'static str, &'static str); 15] {
    [
        (
            "crates/gb-test-runner/data/acid.toml",
            include_str!("../data/acid.toml"),
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
    let startup_mode = parse_manifest_startup_mode(
        source_path,
        &case.id,
        case.startup.as_deref().unwrap_or("skip-boot"),
    );

    CuratedTestRomCase {
        family,
        id: case.id,
        rom: case.rom,
        report_model_suffix: case.report_model_suffix.unwrap_or(false),
        timeout_frames: case.timeout_frames,
        oracle: case.oracle,
        expected: case.expected,
        fixture: case.fixture,
        fixtures: case.fixtures,
        console_model,
        startup_mode,
        execution_mode: case.execution_mode,
        startup_timer_profile: case.startup_timer_profile,
        startup_memory_profile: case.startup_memory_profile,
    }
}

fn parse_manifest_subsystem(source_path: &str, subsystem: &str) -> TestSubsystem {
    match subsystem {
        "Ppu" => TestSubsystem::Ppu,
        "Dma" => TestSubsystem::Dma,
        "Apu" => TestSubsystem::Apu,
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

fn parse_manifest_startup_mode(source_path: &str, case_id: &str, startup: &str) -> StartupMode {
    match startup {
        "skip-boot" => StartupMode::SkipBoot,
        "real-boot" => StartupMode::RealBoot,
        other => {
            panic!("unsupported startup mode {other:?} for curated case {case_id} in {source_path}")
        }
    }
}

fn manifest_case_to_rom_test_case(case: CuratedTestRomCase) -> RomTestCase {
    let CuratedTestRomCase {
        family,
        id,
        rom,
        report_model_suffix: _,
        timeout_frames,
        oracle,
        expected,
        fixture,
        fixtures,
        console_model,
        startup_mode,
        execution_mode,
        startup_timer_profile,
        startup_memory_profile,
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
        "framebuffer-grayscale-fixture" => PassCondition::FramebufferGrayscaleFixture(
            fixture.unwrap_or_else(|| panic!("missing fixture path for case {id}")),
        ),
        "framebuffer-rgb555-fixture" => PassCondition::FramebufferRgb555Fixture(
            fixture.unwrap_or_else(|| panic!("missing fixture path for case {id}")),
        ),
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
        PathBuf::from(&family).join(rom),
        Timeout::Frames(timeout_frames),
        pass_condition,
    )
    .with_external_rom_root_key(TEST_ROM_ROOT_ENV_VAR)
    .with_console_model(console_model)
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

    if let Some(profile) = startup_timer_profile.as_deref() {
        rom_case = match profile {
            "hacktix-cgb-bully-div" => rom_case.with_startup_timer_state(TimerStartupState {
                system_counter: 0x1E74,
                tima: 0x00,
                tma: 0x00,
                tac: 0xF8,
            }),
            other => panic!(
                "unsupported startup timer profile {other:?} for curated case {}",
                rom_case.id
            ),
        };
    }

    if let Some(profile) = startup_memory_profile.as_deref() {
        rom_case = match profile {
            "dmg-boot-trademark-tile" => {
                rom_case.with_startup_memory_writes(dmg_boot_trademark_tile_startup_writes())
            }
            "dmg-boot-logo-vram" => {
                rom_case.with_startup_memory_writes(dmg_boot_logo_vram_startup_writes())
            }
            other => panic!(
                "unsupported startup memory profile {other:?} for curated case {}",
                rom_case.id
            ),
        };
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

fn capture_plan_for_pass_condition(pass_condition: &PassCondition) -> CapturePlan {
    match pass_condition {
        PassCondition::SerialContains(_) | PassCondition::SerialExact(_) => CapturePlan::new()
            .with_capture(CaptureKind::Serial)
            .with_capture(CaptureKind::Snapshot),
        PassCondition::SerialHexExact(_) => CapturePlan::new()
            .with_capture(CaptureKind::SerialHex)
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
        | PassCondition::FramebufferGrayscaleFixture(_)
        | PassCondition::FramebufferRgb555Fixture(_)
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
        | PassCondition::FramebufferGrayscaleFixture(_)
        | PassCondition::FramebufferRgb555Fixture(_)
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

fn dmg_boot_trademark_tile_startup_writes() -> [StartupMemoryWrite; 16] {
    std::array::from_fn(|index| {
        StartupMemoryWrite::new(
            DMG_BOOT_TRADEMARK_TILE_VRAM_START + index as u16,
            DMG_BOOT_TRADEMARK_TILE_BYTES[index],
        )
    })
}

fn dmg_boot_logo_vram_startup_writes() -> Vec<StartupMemoryWrite> {
    let mut writes =
        Vec::with_capacity(DMG_BOOT_LOGO_TILE_BYTES.len() + DMG_BOOT_LOGO_MAP_BYTES.len());

    // Seed the post-boot DMG logo tile bytes plus the logo tilemap rows that
    // BullyGB checks under SkipBoot.
    for (index, byte) in DMG_BOOT_LOGO_TILE_BYTES.iter().copied().enumerate() {
        writes.push(StartupMemoryWrite::new(
            DMG_BOOT_LOGO_TILE_VRAM_START + (index as u16 * 2),
            byte,
        ));
    }
    for (index, byte) in DMG_BOOT_LOGO_MAP_BYTES.iter().copied().enumerate() {
        writes.push(StartupMemoryWrite::new(
            DMG_BOOT_LOGO_MAP_VRAM_START + index as u16,
            byte,
        ));
    }

    writes
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
                .map(move |case| (suite.family.as_str(), case))
        })
        .collect::<Vec<_>>();
    rows.sort_by(
        |(left_default_family, left), (right_default_family, right)| {
            let left_family = left.family.as_deref().unwrap_or(left_default_family);
            let right_family = right.family.as_deref().unwrap_or(right_default_family);
            let left_rank = report_family_rank(left_family);
            let right_rank = report_family_rank(right_family);
            let left_order = manifest_case_order(left_family, &left.rom);
            let right_order = manifest_case_order(right_family, &right.rom);

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

    for (default_family, case) in rows {
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
    let rom = report_rom_display(&case.family, &PathBuf::from(&case.family).join(&case.rom));
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
    curated_source_rom_paths()
        .into_iter()
        .position(|(source_family, source_rom)| source_family == family && source_rom == rom)
}

fn curated_source_rom_paths() -> Vec<(String, PathBuf)> {
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
        .filter_map(|file| curated_required_rom_path(&file.path))
        .collect()
}

fn curated_required_rom_path(path: &Path) -> Option<(String, PathBuf)> {
    if !matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("gb" | "gbc")
    ) {
        return None;
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
        GBEMU_SHOOTOUT_TESTROMS_DIR, MEALYBUG_SAMEBOY_SHOOTOUT_NON_PASS_CASE_IDS,
        PersistedCaseStatus, PersistedSuiteStatus, REPORT_STATUS_FAIL_EMOJI,
        REPORT_STATUS_INFO_EMOJI, REPORT_STATUS_PASS_EMOJI, TEST_ROM_REPORT_FILE_NAME,
        TEST_ROM_ROOT_ENV_VAR, TEST_ROM_STATUS_DIR_NAME, blargg_dmg_curated_suite,
        blargg_dmg_repo_gated_suite, blargg_memory_text_output_spec,
        capture_plan_for_pass_condition, cgb_audio_blargg_suite, cgb_audio_samesuite_suite,
        cgb_boot_div_suite, cgb_boot_hwio_suite, cgb_dma_suite, cgb_ppu_basic_suite,
        cgb_smoke_suite, copy_curated_rom, curated_test_rom_families,
        curated_test_rom_family_suites, curated_test_rom_manifest_texts,
        curated_test_rom_manifests, discover_test_rom_store_root,
        dmg_boot_trademark_tile_startup_writes, failure_artifacts_for_pass_condition,
        load_persisted_suite_status, manifest_case_report_rom_display,
        manifest_case_to_rom_test_case, materialize_curated_test_rom_families,
        materialize_curated_test_rom_store, mealybug_tearoom_dmg_curated_suite,
        mealybug_tearoom_dmg_sameboy_differential_suite, parse_manifest_case,
        parse_manifest_console_model, parse_manifest_subsystem, render_markdown_report,
        report_rom_display, report_status_display, sort_persisted_case_statuses,
        test_rom_store_root, update_curated_test_report,
    };
    use crate::{
        CaptureKind, CapturedArtifacts, PassCondition, RomCaseFailure, RomCaseOutcome,
        RomCaseReport, RomSuiteReport, TestSubsystem, Timeout,
    };
    use gb_core::{ConsoleModel, StartupMode};
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
            for case in &manifest.cases {
                let source_path = root
                    .join(GBEMU_SHOOTOUT_TESTROMS_DIR)
                    .join(&case.family)
                    .join(&case.rom);
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
                && case.startup_mode == StartupMode::RealBoot
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
        assert_eq!(suite.cases[0].startup_mode, StartupMode::RealBoot);
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
    fn cgb_boot_hwio_suite_is_manifest_backed_and_internal_informational() {
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
        assert_eq!(suite.cases[0].startup_mode, StartupMode::RealBoot);
        assert_eq!(
            suite.cases[0].external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert!(matches!(
            suite.cases[0].pass_condition,
            PassCondition::Informational(CaptureKind::Snapshot)
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
        assert_eq!(
            case.startup_timer_state,
            Some(gb_core::TimerStartupState {
                system_counter: 0x1E74,
                tima: 0x00,
                tma: 0x00,
                tac: 0xF8,
            })
        );
        assert_eq!(case.startup_memory_writes.len(), 244);
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
            (
                "cgb-dma-gbc-dma-cont",
                "samesuite/dma/gbc_dma_cont.gb",
                "crates/gb-test-runner/data/fixtures/samesuite/dma/gbc_dma_cont.png",
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
        assert_eq!(last.id, "cgb-audio-samesuite-channel-2-nrx2-speed-change");
        assert_eq!(
            last.rom_path,
            PathBuf::from("samesuite/apu/channel_2/channel_2_nrx2_speed_change.gb")
        );
        assert_eq!(last.timeout, Timeout::Frames(420));

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
            .expect("mealybug startup-memory case should exist");
        assert_eq!(
            mealybug_case.startup_memory_writes,
            dmg_boot_trademark_tile_startup_writes().to_vec()
        );

        let hacktix_case = suites
            .iter()
            .find(|suite| suite.family.as_deref() == Some("hacktix"))
            .and_then(|suite| suite.cases.iter().find(|case| case.id == "hacktix-bully"))
            .expect("hacktix startup-memory case should exist");
        assert_eq!(hacktix_case.startup_memory_writes.len(), 244);

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
                "blargg".to_string(),
                "cpp".to_string(),
                "daid".to_string(),
                "hacktix".to_string(),
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
            for case in &manifest.cases {
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

        sort_persisted_case_statuses("acid", &mut case_statuses);

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
                report_model_suffix: None,
                timeout_frames: 1,
                oracle: "info-framebuffer".to_string(),
                expected: None,
                fixture: None,
                fixtures: None,
                console: None,
                startup: None,
                execution_mode: None,
                startup_timer_profile: None,
                startup_memory_profile: None,
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
            report_model_suffix: false,
            timeout_frames: 1,
            oracle: "unknown".to_string(),
            expected: None,
            fixture: None,
            fixtures: None,
            console_model: ConsoleModel::GameBoy,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: None,
            startup_timer_profile: None,
            startup_memory_profile: None,
        });
    }

    #[test]
    #[should_panic(expected = "unsupported startup timer profile")]
    fn manifest_case_to_rom_test_case_rejects_unknown_startup_timer_profiles() {
        let _ = manifest_case_to_rom_test_case(CuratedTestRomCase {
            family: "hacktix".to_string(),
            id: "bad-timer-profile".to_string(),
            rom: PathBuf::from("bad.gb"),
            report_model_suffix: false,
            timeout_frames: 1,
            oracle: "framebuffer-rgb555-fixture".to_string(),
            expected: None,
            fixture: Some(PathBuf::from("fixture.png")),
            fixtures: None,
            console_model: ConsoleModel::GameBoyColor,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: None,
            startup_timer_profile: Some("unknown-profile".to_string()),
            startup_memory_profile: None,
        });
    }

    #[test]
    #[should_panic(expected = "unsupported startup memory profile")]
    fn manifest_case_to_rom_test_case_rejects_unknown_startup_profiles() {
        let _ = manifest_case_to_rom_test_case(CuratedTestRomCase {
            family: "mealybug-tearoom-tests".to_string(),
            id: "bad-profile".to_string(),
            rom: PathBuf::from("ppu/bad.gb"),
            report_model_suffix: false,
            timeout_frames: 1,
            oracle: "framebuffer-fixture".to_string(),
            expected: None,
            fixture: Some(PathBuf::from("fixture.png")),
            fixtures: None,
            console_model: ConsoleModel::GameBoy,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: None,
            startup_timer_profile: None,
            startup_memory_profile: Some("unknown-profile".to_string()),
        });
    }

    #[test]
    fn report_file_name_stays_stable() {
        assert_eq!(TEST_ROM_REPORT_FILE_NAME, "test-report.md");
    }
}
