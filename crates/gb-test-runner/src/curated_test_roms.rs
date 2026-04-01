use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedTestRomManifestFile {
    version: u32,
    family: String,
    suite_name: String,
    subsystem: String,
    #[serde(rename = "case")]
    cases: Vec<CuratedTestRomCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CuratedTestRomCase {
    id: String,
    rom: PathBuf,
    timeout_frames: u32,
    oracle: String,
    expected: Option<String>,
    fixture: Option<PathBuf>,
    fixtures: Option<Vec<PathBuf>>,
    execution_mode: Option<String>,
    startup_memory_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CuratedTestRomManifest {
    family: String,
    suite_name: String,
    subsystem: TestSubsystem,
    cases: Vec<CuratedTestRomCase>,
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
    rom: String,
    status: String,
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

pub fn mooneye_acceptance_dmg_curated_suite() -> RomSuite {
    manifest_suite("mooneye")
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
    let mut families = curated_test_rom_manifests()
        .into_iter()
        .map(|manifest| manifest.family)
        .collect::<Vec<_>>();
    families.sort();
    families
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

    let mut materialized_families = BTreeSet::new();
    for manifest in curated_test_rom_manifests() {
        if let Some(selected_families) = selected_families
            && !selected_families.contains(manifest.family.as_str())
        {
            continue;
        }

        materialized_families.insert(manifest.family.clone());
        let family_root = store_root.join(&manifest.family);
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

        for case in &manifest.cases {
            let source_path = gbemu_shootout_root
                .join(GBEMU_SHOOTOUT_TESTROMS_DIR)
                .join(&manifest.family)
                .join(&case.rom);
            let target_path = family_root.join(&case.rom);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "failed to create curated ROM parent {}: {error}",
                        parent.display()
                    )
                })?;
            }
            fs::copy(&source_path, &target_path).map_err(|error| {
                format!(
                    "failed to copy curated ROM {} -> {}: {error}",
                    source_path.display(),
                    target_path.display()
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
        let rom = report_rom_display(family, &case.rom_path);
        let status = case.outcome.report_status().to_string();

        if let Some(existing) = merged_case_statuses
            .iter_mut()
            .find(|entry| entry.rom == rom)
        {
            existing.status = status;
        } else {
            merged_case_statuses.push(PersistedCaseStatus { rom, status });
        }
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
        suites.push(persisted);
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
    Ok(Some(persisted))
}

fn manifest_case_order(family: &str, rom: &str) -> Option<usize> {
    curated_test_rom_manifests()
        .into_iter()
        .find(|manifest| manifest.family == family)
        .and_then(|manifest| {
            manifest
                .cases
                .iter()
                .position(|case| case.rom == Path::new(rom))
        })
}

fn sort_persisted_case_statuses(family: &str, case_statuses: &mut [PersistedCaseStatus]) {
    case_statuses.sort_by(|left, right| {
        let left_order = manifest_case_order(family, &left.rom);
        let right_order = manifest_case_order(family, &right.rom);
        (left_order.is_none(), left_order.unwrap_or(usize::MAX))
            .cmp(&(right_order.is_none(), right_order.unwrap_or(usize::MAX)))
            .then_with(|| left.rom.cmp(&right.rom))
    });
}

fn manifest_suite(family: &str) -> RomSuite {
    let manifest = curated_test_rom_manifests()
        .into_iter()
        .find(|manifest| manifest.family == family)
        .unwrap_or_else(|| panic!("missing curated test ROM manifest for family {family}"));

    let mut suite = RomSuite::new(manifest.suite_name, manifest.subsystem).with_family(family);
    for case in manifest.cases {
        suite.push_case(manifest_case_to_rom_test_case(&manifest.family, case));
    }
    suite
}

fn curated_test_rom_manifests() -> Vec<CuratedTestRomManifest> {
    curated_test_rom_manifest_texts()
        .into_iter()
        .map(|(source_path, source_text)| parse_manifest(source_path, source_text))
        .collect()
}

fn curated_test_rom_manifest_texts() -> [(&'static str, &'static str); 7] {
    [
        (
            "crates/gb-test-runner/data/acid.toml",
            include_str!("../data/acid.toml"),
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
        family: parsed.family,
        suite_name: parsed.suite_name,
        subsystem: parse_manifest_subsystem(source_path, &parsed.subsystem),
        cases: parsed.cases,
    }
}

fn parse_manifest_subsystem(source_path: &str, subsystem: &str) -> TestSubsystem {
    match subsystem {
        "Ppu" => TestSubsystem::Ppu,
        "CrossSubsystem" => TestSubsystem::CrossSubsystem,
        other => panic!("unsupported subsystem {other:?} in {source_path}"),
    }
}

fn manifest_case_to_rom_test_case(family: &str, case: CuratedTestRomCase) -> RomTestCase {
    let CuratedTestRomCase {
        id,
        rom,
        timeout_frames,
        oracle,
        expected,
        fixture,
        fixtures,
        execution_mode,
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
        "framebuffer-fixture-set" => PassCondition::FramebufferFixtureSet(
            fixtures.unwrap_or_else(|| panic!("missing fixture paths for case {id}")),
        ),
        other => panic!("unsupported oracle {other:?} for case {id}"),
    };

    let capture_plan = capture_plan_for_pass_condition(&pass_condition);
    let failure_artifacts = failure_artifacts_for_pass_condition(&pass_condition);
    let mut rom_case = RomTestCase::new(
        id,
        PathBuf::from(family).join(rom),
        Timeout::Frames(timeout_frames),
        pass_condition,
    )
    .with_external_rom_root_key(TEST_ROM_ROOT_ENV_VAR)
    .with_capture_plan(capture_plan)
    .with_failure_artifacts(failure_artifacts);

    if let Some(execution_mode) = execution_mode.as_deref() {
        let case_id = rom_case.id.clone();
        rom_case = rom_case.with_execution_mode(parse_manifest_execution_mode(
            family,
            &case_id,
            execution_mode,
        ));
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
        PassCondition::FramebufferFixture(_) | PassCondition::FramebufferFixtureSet(_) => {
            CapturePlan::new()
                .with_capture(CaptureKind::Framebuffer)
                .with_capture(CaptureKind::Snapshot)
        }
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
        PassCondition::FramebufferFixture(_) | PassCondition::FramebufferFixtureSet(_) => {
            FailureArtifactPolicy::new()
                .with_artifact(CaptureKind::Framebuffer)
                .with_artifact(CaptureKind::Snapshot)
        }
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
    let mut ordered_suites = suites.to_vec();
    ordered_suites.sort_by(compare_report_suites);
    let (non_failing_cases, total_cases) = report_summary_counts(&ordered_suites);

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Test Report ({non_failing_cases}/{total_cases})"
    );
    let _ = writeln!(&mut report);
    let _ = writeln!(&mut report, "| family | rom | status |");
    let _ = writeln!(&mut report, "| --- | --- | --- |");

    for suite in &ordered_suites {
        for case in &suite.cases {
            let _ = writeln!(
                &mut report,
                "| {} | {} | {} |",
                suite.family,
                case.rom,
                report_status_display(&case.status)
            );
        }
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
        CuratedTestRomCase, GBEMU_SHOOTOUT_TESTROMS_DIR, PersistedCaseStatus, PersistedSuiteStatus,
        REPORT_STATUS_FAIL_EMOJI, REPORT_STATUS_INFO_EMOJI, REPORT_STATUS_PASS_EMOJI,
        TEST_ROM_REPORT_FILE_NAME, TEST_ROM_ROOT_ENV_VAR, TEST_ROM_STATUS_DIR_NAME,
        blargg_dmg_curated_suite, blargg_dmg_repo_gated_suite, capture_plan_for_pass_condition,
        curated_test_rom_families, curated_test_rom_family_suites, curated_test_rom_manifests,
        discover_test_rom_store_root, dmg_boot_trademark_tile_startup_writes,
        failure_artifacts_for_pass_condition, load_persisted_suite_status,
        manifest_case_to_rom_test_case, materialize_curated_test_rom_families,
        materialize_curated_test_rom_store, parse_manifest_subsystem, render_markdown_report,
        report_rom_display, report_status_display, sort_persisted_case_statuses,
        test_rom_store_root, update_curated_test_report,
    };
    use crate::{
        CaptureKind, CapturedArtifacts, PassCondition, RomCaseFailure, RomCaseOutcome,
        RomCaseReport, RomSuiteReport, TestSubsystem,
    };
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
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

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
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
                    .join(&manifest.family)
                    .join(&case.rom);
                let source_parent = source_path
                    .parent()
                    .expect("curated ROM path should always have a parent");
                fs::create_dir_all(source_parent)
                    .expect("fake shootout ROM parent should be creatable");
                fs::write(
                    &source_path,
                    format!("{}:{}", manifest.family, case.rom.display()),
                )
                .expect("fake shootout ROM should be writable");
            }
        }
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
            ]
        );
    }

    #[test]
    fn discover_test_rom_store_root_prefers_env_then_existing_default_then_none() {
        let workspace_root = unique_temp_dir("discover-root");
        let default_root = test_rom_store_root(&workspace_root);
        fs::create_dir_all(&default_root).expect("default test ROM store should be creatable");

        let _guard = env_lock().lock().expect("env lock should not be poisoned");
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
            let family_root = test_rom_store_root(&workspace_root).join(&manifest.family);
            assert!(!family_root.join("catalog.toml").exists());
            for case in &manifest.cases {
                assert_eq!(
                    fs::read_to_string(family_root.join(&case.rom))
                        .expect("curated ROM should be readable"),
                    format!("{}:{}", manifest.family, case.rom.display())
                );
            }
        }

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
        assert!(error.contains("testroms/acid/which.gb"));

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
        assert!(
            rendered_report.contains(&format!("| acid | which.gb | {REPORT_STATUS_INFO_EMOJI} |"))
        );

        fs::remove_dir_all(workspace_root).expect("temp workspace should be removable");
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
    fn render_markdown_report_orders_present_families_without_placeholders() {
        let rendered = render_markdown_report(&[
            PersistedSuiteStatus {
                version: 1,
                suite_name: "mooneye-acceptance-dmg-curated".to_string(),
                family: "mooneye".to_string(),
                cases: vec![
                    PersistedCaseStatus {
                        rom: "acceptance/div_timing.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                    PersistedCaseStatus {
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
                        rom: "which.gb".to_string(),
                        status: "INFO".to_string(),
                    },
                    PersistedCaseStatus {
                        rom: "dmg-acid2.gb".to_string(),
                        status: "PASS".to_string(),
                    },
                ],
            },
        ]);

        assert!(rendered.starts_with("# Test Report (4/4)\n"));
        let acid_which = rendered
            .find(&format!("| acid | which.gb | {REPORT_STATUS_INFO_EMOJI} |"))
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
        assert!(mooneye_div < mooneye_add);
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
        assert!(
            rendered_report.contains(&format!("| acid | which.gb | {REPORT_STATUS_INFO_EMOJI} |"))
        );
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
                rom: "dmg-acid2.gb".to_string(),
                status: "PASS".to_string(),
            },
            PersistedCaseStatus {
                rom: "which.gb".to_string(),
                status: "INFO".to_string(),
            },
            PersistedCaseStatus {
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
    #[should_panic(expected = "unsupported oracle")]
    fn manifest_case_to_rom_test_case_rejects_unknown_oracles() {
        let _ = manifest_case_to_rom_test_case(
            "blargg",
            CuratedTestRomCase {
                id: "bad-oracle".to_string(),
                rom: PathBuf::from("bad.gb"),
                timeout_frames: 1,
                oracle: "unknown".to_string(),
                expected: None,
                fixture: None,
                fixtures: None,
                execution_mode: None,
                startup_memory_profile: None,
            },
        );
    }

    #[test]
    #[should_panic(expected = "unsupported startup memory profile")]
    fn manifest_case_to_rom_test_case_rejects_unknown_startup_profiles() {
        let _ = manifest_case_to_rom_test_case(
            "mealybug-tearoom-tests",
            CuratedTestRomCase {
                id: "bad-profile".to_string(),
                rom: PathBuf::from("ppu/bad.gb"),
                timeout_frames: 1,
                oracle: "framebuffer-fixture".to_string(),
                expected: None,
                fixture: Some(PathBuf::from("fixture.png")),
                fixtures: None,
                execution_mode: None,
                startup_memory_profile: Some("unknown-profile".to_string()),
            },
        );
    }

    #[test]
    fn report_file_name_stays_stable() {
        assert_eq!(TEST_ROM_REPORT_FILE_NAME, "test-report.md");
    }
}
