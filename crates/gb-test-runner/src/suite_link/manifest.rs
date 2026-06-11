use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use gb_core::{ConsoleModel, Dmg07Port, HardwareRevision, HostPlatform, StartupMode};
use serde::Deserialize;

use crate::oracle::{Oracle, OracleConfig, OracleFixtureRoots};

use super::model::{
    DATA_DIR, LinkParticipant, LinkSuiteCase, LinkSuiteManifest, LinkTopology,
    REPORTS_MANIFEST_PATH, Report, TEST_ROM_STORE_DIR,
};
use super::source::{FamilyTargetRoots, load_family_target_roots};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ReportManifestFile {
    status_dir: Option<PathBuf>,
    artifact_dir: Option<PathBuf>,
    #[serde(rename = "report")]
    reports: Vec<ReportFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ReportFile {
    id: String,
    #[serde(default)]
    local: bool,
    store_dir: PathBuf,
    sources: Option<PathBuf>,
    status_dir: Option<PathBuf>,
    artifact_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LinkSuiteDefaultsFile {
    startup: Option<String>,
    timeout_tcycles: Option<u64>,
    oracle: Option<OracleConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct LinkSuiteManifestFile {
    report: Option<String>,
    suite_name: String,
    family: Option<String>,
    topology: String,
    #[serde(flatten)]
    defaults: LinkSuiteDefaultsFile,
    #[serde(rename = "case")]
    cases: Vec<LinkCaseFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct LinkSuiteManifestHeaderFile {
    suite_name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct LinkCaseFile {
    id: String,
    topology: Option<String>,
    startup: Option<String>,
    timeout_tcycles: Option<u64>,
    #[serde(default)]
    disabled: bool,
    comment: Option<String>,
    oracle: Option<OracleConfig>,
    #[serde(rename = "participant", default)]
    participants: Vec<LinkParticipantFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct LinkParticipantFile {
    id: String,
    rom: PathBuf,
    model: String,
    revision: Option<String>,
    startup: Option<String>,
    adapter_port: Option<String>,
}

pub(super) fn load_reports(workspace_root: &Path) -> Result<Vec<Report>, String> {
    let path = workspace_root.join(REPORTS_MANIFEST_PATH);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read report manifest {}: {error}", path.display()))?;
    let manifest: ReportManifestFile = toml::from_str(&text).map_err(|error| {
        format!(
            "failed to parse report manifest {}: {error}",
            path.display()
        )
    })?;
    let default_status_dir = manifest
        .status_dir
        .unwrap_or_else(|| PathBuf::from(".status"));
    let default_artifact_dir = manifest
        .artifact_dir
        .unwrap_or_else(|| PathBuf::from(".artifacts"));
    let mut reports = Vec::with_capacity(manifest.reports.len());
    for report in manifest.reports {
        match (report.local, &report.sources) {
            (true, Some(_)) => {
                return Err(format!(
                    "local report {:?} must not define sources",
                    report.id
                ));
            }
            (true, None) => {}
            (false, Some(_)) => {}
            (false, None) => {
                return Err(format!(
                    "report {:?} must define sources unless local = true",
                    report.id
                ));
            }
        }
        reports.push(Report {
            id: report.id,
            local: report.local,
            store_dir: report.store_dir,
            sources: report.sources,
            status_dir: report
                .status_dir
                .unwrap_or_else(|| default_status_dir.clone()),
            artifact_dir: report
                .artifact_dir
                .unwrap_or_else(|| default_artifact_dir.clone()),
        });
    }
    Ok(reports)
}

pub(super) fn load_selected_link_suites(
    workspace_root: &Path,
    report: &Report,
    suite_name: Option<&str>,
    case_id: Option<&str>,
) -> Result<Vec<LinkSuiteManifest>, String> {
    let manifest_paths = link_suite_manifest_paths(workspace_root, report)?;
    let family_target_roots = load_family_target_roots(workspace_root, report)?;
    let mut suites = Vec::new();
    for path in manifest_paths {
        let text = read_link_suite_manifest_text(&path)?;
        if let Some(selected_suite_name) = suite_name {
            let header = parse_link_suite_manifest_header(&path, &text)?;
            if header.suite_name != selected_suite_name {
                continue;
            }
        }
        suites.push(parse_link_suite_manifest(
            &path,
            report,
            workspace_root,
            &family_target_roots,
            &text,
        )?);
    }

    if let Some(suite_name) = suite_name
        && suites.is_empty()
    {
        return Err(format!(
            "unknown linked suite {suite_name:?} for report {:?}",
            report.id
        ));
    }

    if let Some(case_id) = case_id {
        for suite in &mut suites {
            suite.cases.retain(|case| case.id == case_id);
        }
        if suites.iter().all(|suite| suite.cases.is_empty()) {
            return Err(format!(
                "unknown case {case_id:?} for linked suite {:?}",
                suite_name.expect("case selection requires suite selection")
            ));
        }
    }

    Ok(suites)
}

fn link_suite_manifest_paths(
    workspace_root: &Path,
    report: &Report,
) -> Result<Vec<PathBuf>, String> {
    let report_data_dir = report_data_dir(workspace_root, report);
    let entries = fs::read_dir(&report_data_dir).map_err(|error| {
        format!(
            "failed to read linked suite manifest directory {}: {error}",
            report_data_dir.display()
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read linked suite manifest directory {}: {error}",
                report_data_dir.display()
            )
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            continue;
        };
        if file_name.ends_with(".link.suite.toml") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn read_link_suite_manifest_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read linked suite manifest {}: {error}",
            path.display()
        )
    })
}

fn parse_link_suite_manifest_header(
    path: &Path,
    text: &str,
) -> Result<LinkSuiteManifestHeaderFile, String> {
    toml::from_str(text).map_err(|error| {
        format!(
            "failed to parse linked suite manifest header {}: {error}",
            path.display()
        )
    })
}

fn parse_link_suite_manifest(
    path: &Path,
    report: &Report,
    workspace_root: &Path,
    family_target_roots: &FamilyTargetRoots,
    text: &str,
) -> Result<LinkSuiteManifest, String> {
    validate_link_suite_manifest_keys(path, text)?;
    let parsed: LinkSuiteManifestFile = toml::from_str(text).map_err(|error| {
        format!(
            "failed to parse linked suite manifest {}: {error}",
            path.display()
        )
    })?;
    validate_link_suite_report(path, report, parsed.report.as_ref())?;
    let family = parsed.family.ok_or_else(|| {
        format!(
            "linked suite manifest {} must define family",
            path.display()
        )
    })?;
    let topology = parse_topology(&parsed.topology)
        .map_err(|error| format!("linked suite manifest {}: {error}", path.display()))?;
    let mut seen_cases = BTreeSet::new();
    let mut cases = Vec::new();
    for case in parsed.cases {
        if !seen_cases.insert(case.id.clone()) {
            return Err(format!(
                "duplicate linked case id {:?} in manifest {}",
                case.id,
                path.display()
            ));
        }
        if case.disabled {
            validate_disabled_case_comment(path, &case)?;
            continue;
        }
        cases.push(parse_case(
            LinkCaseParseContext {
                path,
                workspace_root,
                report,
                family_target_roots,
                family: &family,
                manifest_topology: topology,
                defaults: &parsed.defaults,
            },
            case,
        )?);
    }
    if cases.is_empty() {
        return Err(format!(
            "linked suite manifest {} must define at least one case",
            path.display()
        ));
    }
    Ok(LinkSuiteManifest {
        suite_name: parsed.suite_name,
        family,
        topology,
        cases,
    })
}

fn validate_link_suite_manifest_keys(path: &Path, text: &str) -> Result<(), String> {
    let parsed: toml::Value = toml::from_str(text).map_err(|error| {
        format!(
            "failed to parse linked suite manifest {}: {error}",
            path.display()
        )
    })?;
    let Some(table) = parsed.as_table() else {
        return Ok(());
    };

    validate_link_manifest_table_keys(
        path,
        "linked suite manifest",
        table,
        &[
            "report",
            "suite_name",
            "family",
            "topology",
            "startup",
            "timeout_tcycles",
            "oracle",
            "case",
        ],
    )?;

    let Some(toml::Value::Array(cases)) = table.get("case") else {
        return Ok(());
    };
    for case in cases {
        let toml::Value::Table(case) = case else {
            continue;
        };
        let case_owner = case
            .get("id")
            .and_then(toml::Value::as_str)
            .map(|id| format!("case {id:?}"))
            .unwrap_or_else(|| "case".to_string());
        validate_link_manifest_table_keys(
            path,
            &case_owner,
            case,
            &[
                "id",
                "topology",
                "startup",
                "timeout_tcycles",
                "disabled",
                "comment",
                "oracle",
                "participant",
            ],
        )?;

        let Some(toml::Value::Array(participants)) = case.get("participant") else {
            continue;
        };
        for participant in participants {
            let toml::Value::Table(participant) = participant else {
                continue;
            };
            let participant_owner = participant
                .get("id")
                .and_then(toml::Value::as_str)
                .map(|id| format!("participant {id:?} in {case_owner}"))
                .unwrap_or_else(|| format!("participant in {case_owner}"));
            validate_link_manifest_table_keys(
                path,
                &participant_owner,
                participant,
                &["id", "rom", "model", "revision", "startup", "adapter_port"],
            )?;
        }
    }

    Ok(())
}

fn validate_link_manifest_table_keys(
    path: &Path,
    owner: &str,
    table: &toml::Table,
    supported_keys: &[&str],
) -> Result<(), String> {
    for key in table.keys() {
        if !supported_keys.contains(&key.as_str()) {
            return Err(format!(
                "{owner} in {} uses unsupported key {key:?}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn validate_link_suite_report(
    path: &Path,
    report: &Report,
    declared_report: Option<&String>,
) -> Result<(), String> {
    let declared_report = declared_report.ok_or_else(|| {
        format!(
            "linked suite manifest {} must define report {:?}",
            path.display(),
            report.id
        )
    })?;
    if declared_report != &report.id {
        return Err(format!(
            "linked suite manifest {} declares report {:?}, expected {:?}",
            path.display(),
            declared_report,
            report.id
        ));
    }
    Ok(())
}

fn validate_disabled_case_comment(path: &Path, case: &LinkCaseFile) -> Result<(), String> {
    if case
        .comment
        .as_deref()
        .is_some_and(|comment| !comment.trim().is_empty())
    {
        return Ok(());
    }

    Err(format!(
        "disabled linked case {:?} in {} must include a non-empty comment",
        case.id,
        path.display()
    ))
}

struct LinkCaseParseContext<'a> {
    path: &'a Path,
    workspace_root: &'a Path,
    report: &'a Report,
    family_target_roots: &'a FamilyTargetRoots,
    family: &'a str,
    manifest_topology: LinkTopology,
    defaults: &'a LinkSuiteDefaultsFile,
}

fn parse_case(
    context: LinkCaseParseContext<'_>,
    case: LinkCaseFile,
) -> Result<LinkSuiteCase, String> {
    let topology = match case.topology.as_deref() {
        Some(topology) => parse_topology(topology).map_err(|error| {
            format!("case {:?} in {}: {error}", case.id, context.path.display())
        })?,
        None => context.manifest_topology,
    };
    let timeout_tcycles = case
        .timeout_tcycles
        .or(context.defaults.timeout_tcycles)
        .ok_or_else(|| {
            format!(
                "case {:?} in {} must define timeout_tcycles",
                case.id,
                context.path.display()
            )
        })?;
    if timeout_tcycles == 0 {
        return Err(format!(
            "case {:?} in {} must define a non-zero timeout_tcycles",
            case.id,
            context.path.display()
        ));
    }
    let oracle_config = resolve_oracle_config(context.defaults.oracle.as_ref(), case.oracle)
        .map_err(|error| format!("case {:?} in {}: {error}", case.id, context.path.display()))?;
    oracle_config
        .validate_relative_path_parameter("fixture")
        .map_err(|error| format!("case {:?} in {}: {error}", case.id, context.path.display()))?;
    let target_root = context
        .family_target_roots
        .target_root_for_family(context.family)
        .map_err(|error| format!("case {:?} in {}: {error}", case.id, context.path.display()))?;
    let store_fixture_root =
        fixture_root_for_case(context.workspace_root, context.report, &target_root);
    let local_fixture_root = report_data_dir(context.workspace_root, context.report);
    let oracle = Oracle::from_manifest_with_fixture_roots(
        &oracle_config,
        OracleFixtureRoots {
            store: &store_fixture_root,
            local: &local_fixture_root,
        },
    )
    .map_err(|error| format!("case {:?} in {}: {error}", case.id, context.path.display()))?;

    let case_startup = case.startup.or_else(|| context.defaults.startup.clone());
    let mut seen_participants = BTreeSet::new();
    let mut participants = Vec::new();
    for participant in case.participants {
        if !seen_participants.insert(participant.id.clone()) {
            return Err(format!(
                "duplicate participant id {:?} in case {:?} in {}",
                participant.id,
                case.id,
                context.path.display()
            ));
        }
        participants.push(parse_participant(
            context.path,
            &case.id,
            case_startup.as_deref(),
            participant,
        )?);
    }
    validate_topology_participants(context.path, &case.id, topology, &participants)?;

    Ok(LinkSuiteCase {
        id: case.id,
        topology,
        timeout_tcycles,
        target_root,
        participants,
        oracle,
    })
}

fn parse_participant(
    path: &Path,
    case_id: &str,
    case_startup: Option<&str>,
    participant: LinkParticipantFile,
) -> Result<LinkParticipant, String> {
    validate_relative_manifest_path(&participant.rom).map_err(|error| {
        format!(
            "participant {:?} in case {case_id:?} in {}: {error}",
            participant.id,
            path.display()
        )
    })?;
    let model_profile = parse_model_profile(&participant.model).map_err(|error| {
        format!(
            "participant {:?} in case {case_id:?} in {}: {error}",
            participant.id,
            path.display()
        )
    })?;
    let hardware_revision = match participant.revision.as_deref() {
        Some(revision) => parse_hardware_revision(revision).map_err(|error| {
            format!(
                "participant {:?} in case {case_id:?} in {}: {error}",
                participant.id,
                path.display()
            )
        })?,
        None => model_profile.console_model.default_revision(),
    };
    if !model_profile
        .console_model
        .supports_revision_on_host(model_profile.host_platform, hardware_revision)
    {
        return Err(format!(
            "participant {:?} in case {case_id:?} in {}: model {:?} does not support revision {:?}",
            participant.id,
            path.display(),
            participant.model,
            hardware_revision
        ));
    }
    let startup_mode = parse_startup_mode(
        participant
            .startup
            .as_deref()
            .or(case_startup)
            .unwrap_or("skip-boot"),
    )
    .map_err(|error| {
        format!(
            "participant {:?} in case {case_id:?} in {}: {error}",
            participant.id,
            path.display()
        )
    })?;
    let adapter_port = match participant.adapter_port.as_deref() {
        Some(port) => Some(Dmg07Port::from_manifest_name(port).ok_or_else(|| {
            format!(
                "participant {:?} in case {case_id:?} in {} uses unsupported adapter_port {:?}",
                participant.id,
                path.display(),
                port
            )
        })?),
        None => None,
    };
    Ok(LinkParticipant {
        id: participant.id,
        rom: participant.rom,
        console_model: model_profile.console_model,
        hardware_revision,
        host_platform: model_profile.host_platform,
        startup_mode,
        adapter_port,
    })
}

fn validate_topology_participants(
    path: &Path,
    case_id: &str,
    topology: LinkTopology,
    participants: &[LinkParticipant],
) -> Result<(), String> {
    match topology {
        LinkTopology::Dmg04 => {
            if participants.len() != 2 {
                return Err(format!(
                    "case {case_id:?} in {} topology \"dmg04\" requires exactly 2 participants",
                    path.display()
                ));
            }
            if let Some(participant) = participants
                .iter()
                .find(|participant| participant.adapter_port.is_some())
            {
                return Err(format!(
                    "participant {:?} in case {case_id:?} in {} must not define adapter_port for topology \"dmg04\"",
                    participant.id,
                    path.display()
                ));
            }
        }
        LinkTopology::CgbIr => {
            if participants.len() != 2 {
                return Err(format!(
                    "case {case_id:?} in {} topology \"cgb-ir\" requires exactly 2 participants",
                    path.display()
                ));
            }
            for participant in participants {
                if participant.console_model != ConsoleModel::GameBoyColor {
                    return Err(format!(
                        "participant {:?} in case {case_id:?} in {} topology \"cgb-ir\" requires model \"cgb\"",
                        participant.id,
                        path.display()
                    ));
                }
                if participant.adapter_port.is_some() {
                    return Err(format!(
                        "participant {:?} in case {case_id:?} in {} must not define adapter_port for topology \"cgb-ir\"",
                        participant.id,
                        path.display()
                    ));
                }
            }
        }
        LinkTopology::Dmg07 => {
            if participants.len() < 2 || participants.len() > 4 {
                return Err(format!(
                    "case {case_id:?} in {} topology \"dmg07\" requires 2 to 4 participants",
                    path.display()
                ));
            }
            for participant in participants {
                if participant.adapter_port.is_none() {
                    return Err(format!(
                        "participant {:?} in case {case_id:?} in {} topology \"dmg07\" requires adapter_port",
                        participant.id,
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn resolve_oracle_config(
    default: Option<&OracleConfig>,
    case: Option<OracleConfig>,
) -> Result<OracleConfig, String> {
    match (default, case) {
        (_, Some(case)) if case.has_kind() => Ok(case),
        (Some(default), Some(case)) => case.with_defaults(default),
        (Some(default), None) => Ok(default.clone()),
        (None, Some(case)) => Ok(case),
        (None, None) => Err("case must define oracle or inherit a suite oracle".to_string()),
    }
}

fn parse_topology(topology: &str) -> Result<LinkTopology, String> {
    match topology {
        "dmg04" => Ok(LinkTopology::Dmg04),
        "dmg07" => Ok(LinkTopology::Dmg07),
        "cgb-ir" => Ok(LinkTopology::CgbIr),
        other => Err(format!("unsupported topology {other:?}")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ModelProfile {
    console_model: ConsoleModel,
    host_platform: HostPlatform,
}

fn parse_model_profile(model: &str) -> Result<ModelProfile, String> {
    match model {
        "dmg" => Ok(ModelProfile {
            console_model: ConsoleModel::GameBoy,
            host_platform: HostPlatform::Handheld,
        }),
        "mgb" => Ok(ModelProfile {
            console_model: ConsoleModel::GameBoyPocket,
            host_platform: HostPlatform::Handheld,
        }),
        "cgb" => Ok(ModelProfile {
            console_model: ConsoleModel::GameBoyColor,
            host_platform: HostPlatform::Handheld,
        }),
        "agb" => Ok(ModelProfile {
            console_model: ConsoleModel::GameBoyAdvance,
            host_platform: HostPlatform::Handheld,
        }),
        "sgb" => Ok(ModelProfile {
            console_model: ConsoleModel::GameBoy,
            host_platform: HostPlatform::Sgb,
        }),
        "sgb2" => Ok(ModelProfile {
            console_model: ConsoleModel::GameBoy,
            host_platform: HostPlatform::Sgb2,
        }),
        other => Err(format!("unsupported model {other:?}")),
    }
}

fn parse_hardware_revision(revision: &str) -> Result<HardwareRevision, String> {
    match revision {
        "dmg-cpu-0" => Ok(HardwareRevision::DmgCpu0),
        "dmg-cpu-a" => Ok(HardwareRevision::DmgCpuA),
        "dmg-cpu-b" => Ok(HardwareRevision::DmgCpuB),
        "dmg-cpu-c" => Ok(HardwareRevision::DmgCpuC),
        "cpu-mgb" => Ok(HardwareRevision::CpuMgb),
        "cpu-cgb-0" => Ok(HardwareRevision::CpuCgb0),
        "cpu-cgb-a" => Ok(HardwareRevision::CpuCgbA),
        "cpu-cgb-b" => Ok(HardwareRevision::CpuCgbB),
        "cpu-cgb-c" => Ok(HardwareRevision::CpuCgbC),
        "cpu-cgb-d" => Ok(HardwareRevision::CpuCgbD),
        "cpu-cgb-e" => Ok(HardwareRevision::CpuCgbE),
        "cpu-agb-0" => Ok(HardwareRevision::CpuAgb0),
        "cpu-agb-a" => Ok(HardwareRevision::CpuAgbA),
        other => Err(format!("unsupported revision {other:?}")),
    }
}

fn parse_startup_mode(startup: &str) -> Result<StartupMode, String> {
    match startup {
        "skip-boot" => Ok(StartupMode::SkipBoot),
        "custom-boot" => Ok(StartupMode::CustomBoot),
        "real-boot" => Ok(StartupMode::RealBoot),
        other => Err(format!("unsupported startup {other:?}")),
    }
}

fn validate_relative_manifest_path(path: &Path) -> Result<(), String> {
    if path.is_absolute() {
        return Err(format!(
            "path must be relative and confined to the linked report data directory: {}",
            path.display()
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::ParentDir => {
                return Err(format!("path must not contain '..': {}", path.display()));
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "path must be normalized and confined to the linked report data directory: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn report_data_dir(workspace_root: &Path, report: &Report) -> PathBuf {
    if report.local {
        return workspace_root.join(DATA_DIR).join(&report.store_dir);
    }
    let source_parent = report
        .sources
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new(""));
    workspace_root.join(DATA_DIR).join(source_parent)
}

fn fixture_root_for_case(workspace_root: &Path, report: &Report, target_root: &Path) -> PathBuf {
    if report.local {
        return report_data_dir(workspace_root, report);
    }
    let mut root = workspace_root.join(TEST_ROM_STORE_DIR);
    if !report.store_dir.as_os_str().is_empty() {
        root = root.join(&report.store_dir);
    }
    root.join(target_root)
}

#[cfg(test)]
pub(super) fn parse_link_suite_manifest_for_test(
    path: &Path,
    report_id: &str,
    text: &str,
) -> Result<LinkSuiteManifest, String> {
    let report = Report {
        id: report_id.to_string(),
        local: true,
        store_dir: PathBuf::from(report_id),
        sources: None,
        status_dir: PathBuf::from(".status"),
        artifact_dir: PathBuf::from(".artifacts"),
    };
    parse_link_suite_manifest(
        path,
        &report,
        Path::new(""),
        &FamilyTargetRoots::fallback_for_test(),
        text,
    )
}

#[cfg(test)]
pub(super) fn load_selected_link_suites_for_test(
    workspace_root: &Path,
    report: &Report,
    suite_name: Option<&str>,
    case_id: Option<&str>,
) -> Result<Vec<LinkSuiteManifest>, String> {
    load_selected_link_suites(workspace_root, report, suite_name, case_id)
}
