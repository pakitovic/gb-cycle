use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use gb_core::{ConsoleModel, ExecutionMode, HostPlatform, JoypadButton, StartupMode};

use crate::oracle::{Oracle, OracleConfig};

use super::model::{
    DATA_DIR, REPORTS_MANIFEST_PATH, Report, SuiteCase, SuiteManifest, SuiteStimulus,
    SuiteStimulusTime, TEST_ROM_STORE_DIR,
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
    store_dir: PathBuf,
    sources: PathBuf,
    status_dir: Option<PathBuf>,
    artifact_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SuiteCaseDefaultsFile {
    family: Option<String>,
    console: Option<String>,
    startup: Option<String>,
    execution_mode: Option<String>,
    timeout_frames: Option<u32>,
    oracle: Option<OracleConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct SuiteManifestFile {
    family: Option<String>,
    suite_name: String,
    report: Option<String>,
    #[serde(flatten)]
    defaults: SuiteCaseDefaultsFile,
    #[serde(rename = "case")]
    cases: Vec<SuiteCaseFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct SuiteManifestHeaderFile {
    suite_name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SuiteCaseFile {
    family: Option<String>,
    id: String,
    rom: PathBuf,
    console: Option<String>,
    startup: Option<String>,
    execution_mode: Option<String>,
    timeout_frames: Option<u32>,
    #[serde(rename = "stimulus", default)]
    stimuli: Vec<SuiteStimulusFile>,
    #[serde(default)]
    disabled: bool,
    comment: Option<String>,
    oracle: Option<OracleConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct SuiteStimulusFile {
    tcycle: Option<u64>,
    frame: Option<u32>,
    button: String,
    pressed: bool,
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
    Ok(manifest
        .reports
        .into_iter()
        .map(|report| Report {
            id: report.id,
            store_dir: report.store_dir,
            sources: report.sources,
            status_dir: report
                .status_dir
                .unwrap_or_else(|| default_status_dir.clone()),
            artifact_dir: report
                .artifact_dir
                .unwrap_or_else(|| default_artifact_dir.clone()),
        })
        .collect())
}

pub(super) fn load_selected_suites(
    workspace_root: &Path,
    report: &Report,
    suite_name: Option<&str>,
    case_id: Option<&str>,
) -> Result<Vec<SuiteManifest>, String> {
    let manifest_paths = suite_manifest_paths(workspace_root, report)?;
    let family_target_roots = load_family_target_roots(workspace_root, report)?;
    let mut suites = Vec::new();
    for path in manifest_paths {
        let text = read_suite_manifest_text(&path)?;
        if let Some(selected_suite_name) = suite_name {
            let header = parse_suite_manifest_header(&path, &text)?;
            if header.suite_name != selected_suite_name {
                continue;
            }
        }
        suites.push(parse_suite_manifest(
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
            "unknown suite {suite_name:?} for report {:?}",
            report.id
        ));
    }

    if let Some(case_id) = case_id {
        for suite in &mut suites {
            suite.cases.retain(|case| case.id == case_id);
        }
        if suites.iter().all(|suite| suite.cases.is_empty()) {
            return Err(format!(
                "unknown case {case_id:?} for suite {:?}",
                suite_name.expect("case selection requires suite selection")
            ));
        }
    }

    Ok(suites)
}

pub(super) fn load_selected_suite_families(
    workspace_root: &Path,
    report: &Report,
    suite_name: Option<&str>,
    case_id: Option<&str>,
) -> Result<Vec<String>, String> {
    let manifest_paths = suite_manifest_paths(workspace_root, report)?;
    let mut suites_seen = 0;
    let mut selected_families = Vec::new();
    let mut selected_family_set = BTreeSet::new();
    for path in manifest_paths {
        let text = read_suite_manifest_text(&path)?;
        let parsed: SuiteManifestFile = toml::from_str(&text).map_err(|error| {
            format!("failed to parse suite manifest {}: {error}", path.display())
        })?;
        if let Some(selected_suite_name) = suite_name
            && parsed.suite_name != selected_suite_name
        {
            continue;
        }
        validate_suite_report(&path, report, parsed.report.as_ref())?;
        suites_seen += 1;
        let manifest_family = parsed
            .family
            .clone()
            .or_else(|| parsed.defaults.family.clone())
            .ok_or_else(|| format!("suite manifest {} must define family", path.display()))?;
        for case in parsed.cases {
            if case.disabled {
                validate_disabled_case_comment(&path, &case)?;
                continue;
            }
            if let Some(selected_case_id) = case_id
                && case.id != selected_case_id
            {
                continue;
            }
            let family = case
                .family
                .or_else(|| parsed.defaults.family.clone())
                .unwrap_or_else(|| manifest_family.clone());
            if selected_family_set.insert(family.clone()) {
                selected_families.push(family);
            }
        }
    }

    if let Some(suite_name) = suite_name
        && suites_seen == 0
    {
        return Err(format!(
            "unknown suite {suite_name:?} for report {:?}",
            report.id
        ));
    }
    if let Some(case_id) = case_id
        && selected_families.is_empty()
    {
        return Err(format!(
            "unknown case {case_id:?} for suite {:?}",
            suite_name.expect("case selection requires suite selection")
        ));
    }
    Ok(selected_families)
}

fn suite_manifest_paths(workspace_root: &Path, report: &Report) -> Result<Vec<PathBuf>, String> {
    let report_data_dir = report_data_dir(workspace_root, report);
    let entries = fs::read_dir(&report_data_dir).map_err(|error| {
        format!(
            "failed to read suite manifest directory {}: {error}",
            report_data_dir.display()
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read suite manifest directory {}: {error}",
                report_data_dir.display()
            )
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            continue;
        };
        if !file_name.ends_with(".suite.toml") {
            continue;
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

fn report_data_dir(workspace_root: &Path, report: &Report) -> PathBuf {
    let source_parent = report.sources.parent().unwrap_or_else(|| Path::new(""));
    workspace_root.join(DATA_DIR).join(source_parent)
}

fn read_suite_manifest_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|error| format!("failed to read suite manifest {}: {error}", path.display()))
}

fn parse_suite_manifest_header(path: &Path, text: &str) -> Result<SuiteManifestHeaderFile, String> {
    toml::from_str(text).map_err(|error| {
        format!(
            "failed to parse suite manifest header {}: {error}",
            path.display()
        )
    })
}

fn parse_suite_manifest(
    path: &Path,
    report: &Report,
    workspace_root: &Path,
    family_target_roots: &FamilyTargetRoots,
    text: &str,
) -> Result<SuiteManifest, String> {
    let parsed: SuiteManifestFile = toml::from_str(text)
        .map_err(|error| format!("failed to parse suite manifest {}: {error}", path.display()))?;
    validate_suite_report(path, report, parsed.report.as_ref())?;
    let manifest_family = parsed
        .family
        .clone()
        .or_else(|| parsed.defaults.family.clone())
        .ok_or_else(|| format!("suite manifest {} must define family", path.display()))?;
    let mut seen_cases = BTreeSet::new();
    let mut cases = Vec::new();
    for case in parsed.cases {
        if !seen_cases.insert(case.id.clone()) {
            return Err(format!(
                "duplicate case id {:?} in suite manifest {}",
                case.id,
                path.display()
            ));
        }
        if case.disabled {
            validate_disabled_case_comment(path, &case)?;
            continue;
        }
        cases.push(parse_case(
            path,
            workspace_root,
            report,
            family_target_roots,
            &manifest_family,
            &parsed.defaults,
            case,
        )?);
    }
    if cases.is_empty() {
        return Err(format!(
            "suite manifest {} must define at least one case",
            path.display()
        ));
    }

    Ok(SuiteManifest {
        suite_name: parsed.suite_name,
        family: manifest_family,
        cases,
    })
}

fn validate_suite_report(
    path: &Path,
    report: &Report,
    declared_report: Option<&String>,
) -> Result<(), String> {
    let declared_report = declared_report.ok_or_else(|| {
        format!(
            "suite manifest {} must define report {:?}",
            path.display(),
            report.id
        )
    })?;
    if declared_report != &report.id {
        return Err(format!(
            "suite manifest {} declares report {:?}, expected {:?}",
            path.display(),
            declared_report,
            report.id
        ));
    }
    Ok(())
}

fn validate_disabled_case_comment(path: &Path, case: &SuiteCaseFile) -> Result<(), String> {
    if case
        .comment
        .as_deref()
        .is_some_and(|comment| !comment.trim().is_empty())
    {
        return Ok(());
    }

    Err(format!(
        "disabled case {:?} in {} must include a non-empty comment",
        case.id,
        path.display()
    ))
}

fn parse_case(
    path: &Path,
    workspace_root: &Path,
    report: &Report,
    family_target_roots: &FamilyTargetRoots,
    manifest_family: &str,
    defaults: &SuiteCaseDefaultsFile,
    case: SuiteCaseFile,
) -> Result<SuiteCase, String> {
    let family = case
        .family
        .or_else(|| defaults.family.clone())
        .unwrap_or_else(|| manifest_family.to_string());
    let console = case
        .console
        .or_else(|| defaults.console.clone())
        .ok_or_else(|| {
            format!(
                "case {:?} in {} must define console",
                case.id,
                path.display()
            )
        })?;
    let console_profile = parse_console_profile(&console)
        .map_err(|error| format!("case {:?} in {}: {error}", case.id, path.display()))?;
    let startup_mode = match case.startup.or_else(|| defaults.startup.clone()).as_deref() {
        Some(startup) => parse_startup_mode(startup)
            .map_err(|error| format!("case {:?} in {}: {error}", case.id, path.display()))?,
        None => StartupMode::SkipBoot,
    };
    let execution_mode = match case
        .execution_mode
        .or_else(|| defaults.execution_mode.clone())
        .as_deref()
    {
        Some(execution_mode) => parse_execution_mode(execution_mode)
            .map_err(|error| format!("case {:?} in {}: {error}", case.id, path.display()))?,
        None => ExecutionMode::Strict,
    };
    let timeout_frames = case
        .timeout_frames
        .or(defaults.timeout_frames)
        .ok_or_else(|| {
            format!(
                "case {:?} in {} must define timeout_frames",
                case.id,
                path.display()
            )
        })?;
    if timeout_frames == 0 {
        return Err(format!(
            "case {:?} in {} must define a non-zero timeout_frames",
            case.id,
            path.display()
        ));
    }
    let oracle_config = resolve_oracle_config(defaults.oracle.as_ref(), case.oracle)
        .map_err(|error| format!("case {:?} in {}: {error}", case.id, path.display()))?;
    let target_root = family_target_roots
        .target_root_for_family(&family)
        .map_err(|error| format!("case {:?} in {}: {error}", case.id, path.display()))?;
    let fixture_root = fixture_root_for_case(workspace_root, report, &target_root);
    let oracle = Oracle::from_manifest_with_fixture_root(&oracle_config, &fixture_root)
        .map_err(|error| format!("case {:?} in {}: {error}", case.id, path.display()))?;
    let stimuli = case
        .stimuli
        .into_iter()
        .map(|stimulus| parse_stimulus(&case.id, path, stimulus))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SuiteCase {
        id: case.id,
        family,
        rom: case.rom,
        target_root,
        console_model: console_profile.console_model,
        host_platform: console_profile.host_platform,
        execution_mode,
        startup_mode,
        timeout_frames,
        stimuli,
        oracle,
    })
}

fn parse_stimulus(
    case_id: &str,
    path: &Path,
    stimulus: SuiteStimulusFile,
) -> Result<SuiteStimulus, String> {
    let when = match (stimulus.tcycle, stimulus.frame) {
        (Some(tcycle), None) => SuiteStimulusTime::TCycle(tcycle),
        (None, Some(frame)) => SuiteStimulusTime::Frame(frame),
        (Some(_), Some(_)) => {
            return Err(format!(
                "case {case_id:?} in {} stimulus must define either tcycle or frame, not both",
                path.display()
            ));
        }
        (None, None) => {
            return Err(format!(
                "case {case_id:?} in {} stimulus must define tcycle or frame",
                path.display()
            ));
        }
    };
    let button = parse_joypad_button(&stimulus.button)
        .map_err(|error| format!("case {case_id:?} in {}: {error}", path.display()))?;
    Ok(SuiteStimulus {
        when,
        button,
        pressed: stimulus.pressed,
    })
}

fn parse_joypad_button(button: &str) -> Result<JoypadButton, String> {
    match button {
        "right" => Ok(JoypadButton::Right),
        "left" => Ok(JoypadButton::Left),
        "up" => Ok(JoypadButton::Up),
        "down" => Ok(JoypadButton::Down),
        "a" => Ok(JoypadButton::A),
        "b" => Ok(JoypadButton::B),
        "select" => Ok(JoypadButton::Select),
        "start" => Ok(JoypadButton::Start),
        other => Err(format!("unsupported joypad button {other:?}")),
    }
}

fn resolve_oracle_config(
    default: Option<&OracleConfig>,
    case: Option<OracleConfig>,
) -> Result<OracleConfig, String> {
    match case {
        Some(case) if case.has_kind() => Ok(case),
        Some(case) => {
            let default = default
                .ok_or_else(|| "oracle override requires a global oracle with type".to_string())?;
            case.with_defaults(default)
        }
        None => {
            let default = default
                .cloned()
                .ok_or_else(|| "must define oracle".to_string())?;
            if default.has_kind() {
                Ok(default)
            } else {
                Err("global oracle must define type".to_string())
            }
        }
    }
}

fn fixture_root_for_case(workspace_root: &Path, report: &Report, target_root: &Path) -> PathBuf {
    workspace_root
        .join(TEST_ROM_STORE_DIR)
        .join(&report.store_dir)
        .join(target_root)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConsoleProfile {
    console_model: ConsoleModel,
    host_platform: HostPlatform,
}

fn parse_console_profile(console: &str) -> Result<ConsoleProfile, String> {
    match console {
        "dmg" => Ok(ConsoleProfile {
            console_model: ConsoleModel::GameBoy,
            host_platform: HostPlatform::Handheld,
        }),
        "cgb" => Ok(ConsoleProfile {
            console_model: ConsoleModel::GameBoyColor,
            host_platform: HostPlatform::Handheld,
        }),
        "sgb" => Ok(ConsoleProfile {
            console_model: ConsoleModel::GameBoy,
            host_platform: HostPlatform::Sgb,
        }),
        "sgb2" => Ok(ConsoleProfile {
            console_model: ConsoleModel::GameBoy,
            host_platform: HostPlatform::Sgb2,
        }),
        other => Err(format!("unsupported console {other:?}")),
    }
}

fn parse_execution_mode(execution_mode: &str) -> Result<ExecutionMode, String> {
    match execution_mode {
        "strict" => Ok(ExecutionMode::Strict),
        "permissive" => Ok(ExecutionMode::Permissive),
        "experimental" => Ok(ExecutionMode::Experimental),
        other => Err(format!("unsupported execution_mode {other:?}")),
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

#[cfg(test)]
pub(super) fn parse_suite_manifest_for_test(
    path: &Path,
    report_id: &str,
    text: &str,
) -> Result<SuiteManifest, String> {
    let report = Report {
        id: report_id.to_string(),
        store_dir: PathBuf::from(report_id),
        sources: PathBuf::from(format!("{report_id}/sources.report.toml")),
        status_dir: PathBuf::from(".status"),
        artifact_dir: PathBuf::from(".artifacts"),
    };
    parse_suite_manifest(
        path,
        &report,
        Path::new(""),
        &super::source::fallback_family_target_roots_for_test(),
        text,
    )
}
