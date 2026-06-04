use std::cmp::Ordering;
use std::collections::{BTreeMap, btree_map::Entry};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::model::{
    DATA_DIR, PersistedSuiteStatus, Report, ReportDocument, ReportRow, TEST_ROM_STORE_DIR,
    is_non_failing_status, report_status_display,
};

const REPORT_CONSOLE_SUFFIXES: [(&str, usize); 4] =
    [(" (DMG)", 0), (" (GBC)", 1), (" (SGB)", 2), (" (SGB2)", 3)];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SourceManifestFile {
    #[serde(default, rename = "source")]
    sources: Vec<SourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SourceFile {
    #[serde(default, rename = "family")]
    families: Vec<SourceFamilyFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SourceFamilyFile {
    id: String,
    #[serde(default, rename = "file")]
    files: Vec<SourceFamilyFileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SourceFamilyFileEntry {
    target: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ReportSourceOrder {
    rom_ranks: BTreeMap<String, BTreeMap<String, usize>>,
}

pub(super) fn load_statuses(
    workspace_root: &Path,
    report: &Report,
) -> Result<Vec<PersistedSuiteStatus>, String> {
    let status_root = status_root_for_report(workspace_root, report);
    let status_files = status_files(&status_root)?;
    let mut statuses = Vec::with_capacity(status_files.len());
    for path in status_files {
        let text = fs::read_to_string(&path).map_err(|error| {
            format!("failed to read test ROM status {}: {error}", path.display())
        })?;
        let status: PersistedSuiteStatus = toml::from_str(&text).map_err(|error| {
            format!(
                "failed to parse test ROM status {}: {error}",
                path.display()
            )
        })?;
        statuses.push(status);
    }
    Ok(statuses)
}

pub(super) fn build_report_document(
    workspace_root: &Path,
    report: &Report,
    statuses: Vec<PersistedSuiteStatus>,
) -> Result<ReportDocument, String> {
    let mut rows = Vec::new();
    let mut non_failing_cases = 0;
    let mut total_cases = 0;
    for suite in statuses {
        for (case_index, case) in suite.cases.into_iter().enumerate() {
            report_status_display(&case.status)?;
            total_cases += 1;
            if is_non_failing_status(&case.status) {
                non_failing_cases += 1;
            }
            rows.push(ReportRow {
                family: case.family.unwrap_or_else(|| suite.family.clone()),
                rom: case.rom,
                status: case.status,
                suite_name: suite.suite_name.clone(),
                case_index,
            });
        }
    }
    let source_order = load_report_source_order(workspace_root, report)?;
    rows.sort_by(|left, right| {
        compare_report_rows(left, right, report.family_order.as_deref(), &source_order)
    });

    Ok(ReportDocument {
        report_id: report.id.clone(),
        command: format!("cargo rom-report {}", report.id),
        non_failing_cases,
        total_cases,
        rows,
    })
}

pub(super) fn store_root_for_report(workspace_root: &Path, report: &Report) -> PathBuf {
    workspace_root
        .join(TEST_ROM_STORE_DIR)
        .join(&report.store_dir)
}

fn status_files(status_root: &Path) -> Result<Vec<PathBuf>, String> {
    if !status_root.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(status_root).map_err(|error| {
        format!(
            "failed to read test ROM status directory {}: {error}",
            status_root.display()
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read test ROM status entry in {}: {error}",
                status_root.display()
            )
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("toml") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn compare_report_rows(
    left: &ReportRow,
    right: &ReportRow,
    family_order: Option<&[String]>,
    source_order: &ReportSourceOrder,
) -> Ordering {
    let left_rank = report_family_rank(&left.family, family_order);
    let right_rank = report_family_rank(&right.family, family_order);
    let left_source_rank = source_order.rank(&left.family, &left.rom);
    let right_source_rank = source_order.rank(&right.family, &right.rom);
    (left_rank.is_none(), left_rank.unwrap_or(usize::MAX))
        .cmp(&(right_rank.is_none(), right_rank.unwrap_or(usize::MAX)))
        .then_with(|| left.family.cmp(&right.family))
        .then_with(|| {
            (
                left_source_rank.is_none(),
                left_source_rank.unwrap_or(usize::MAX),
            )
                .cmp(&(
                    right_source_rank.is_none(),
                    right_source_rank.unwrap_or(usize::MAX),
                ))
        })
        .then_with(|| compare_report_console_variant(left, right))
        .then_with(|| left.suite_name.cmp(&right.suite_name))
        .then_with(|| left.case_index.cmp(&right.case_index))
        .then_with(|| left.rom.cmp(&right.rom))
}

fn report_family_rank(family: &str, family_order: Option<&[String]>) -> Option<usize> {
    family_order.and_then(|order| order.iter().position(|known| known == family))
}

fn load_report_source_order(
    workspace_root: &Path,
    report: &Report,
) -> Result<ReportSourceOrder, String> {
    let Some(sources) = &report.sources else {
        return Ok(ReportSourceOrder::default());
    };
    let path = workspace_root.join(DATA_DIR).join(sources);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read source manifest {}: {error}", path.display()))?;
    let manifest: SourceManifestFile = toml::from_str(&text).map_err(|error| {
        format!(
            "failed to parse source manifest {}: {error}",
            path.display()
        )
    })?;
    Ok(source_order_from_manifest(&manifest))
}

fn source_order_from_manifest(manifest: &SourceManifestFile) -> ReportSourceOrder {
    let mut source_order = ReportSourceOrder::default();
    let mut family_ranks = BTreeMap::<String, usize>::new();
    for source in &manifest.sources {
        for family in &source.families {
            let rank = family_ranks.entry(family.id.clone()).or_default();
            for file in &family.files {
                if !is_rom_source_target(&file.target) {
                    continue;
                }
                let family_order = source_order.rom_ranks.entry(family.id.clone()).or_default();
                if let Entry::Vacant(entry) = family_order.entry(source_target_key(&file.target)) {
                    entry.insert(*rank);
                }
                *rank += 1;
            }
        }
    }
    source_order
}

impl ReportSourceOrder {
    fn rank(&self, family: &str, rom: &str) -> Option<usize> {
        self.rom_ranks
            .get(family)
            .and_then(|family_order| family_order.get(report_rom_key(rom)))
            .copied()
    }
}

fn compare_report_console_variant(left: &ReportRow, right: &ReportRow) -> Ordering {
    if report_rom_key(&left.rom) != report_rom_key(&right.rom) {
        return Ordering::Equal;
    }
    report_console_variant_rank(&left.rom).cmp(&report_console_variant_rank(&right.rom))
}

fn report_rom_key(rom: &str) -> &str {
    for (suffix, _) in REPORT_CONSOLE_SUFFIXES {
        if let Some(base) = rom.strip_suffix(suffix) {
            return base;
        }
    }
    rom
}

fn report_console_variant_rank(rom: &str) -> usize {
    REPORT_CONSOLE_SUFFIXES
        .iter()
        .find_map(|(suffix, rank)| rom.ends_with(suffix).then_some(*rank))
        .unwrap_or(usize::MAX)
}

fn is_rom_source_target(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "gb" | "gbc"))
        .unwrap_or(false)
}

fn source_target_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn status_root_for_report(workspace_root: &Path, report: &Report) -> PathBuf {
    store_root_for_report(workspace_root, report).join(&report.status_dir)
}
