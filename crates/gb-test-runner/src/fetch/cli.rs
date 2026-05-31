use super::manifest::Report;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FetchAction {
    ShowHelp,
    Fetch(FetchRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FetchRequest {
    pub(super) report_id: Option<String>,
    pub(super) requested_families: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FetchOptions<'a> {
    pub(super) report: &'a Report,
    pub(super) requested_families: Vec<String>,
}

pub(super) fn parse_fetch_arguments<I, S>(arguments: I) -> Result<FetchAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut report_id = None;
    let mut requested_families = Vec::new();

    for argument in arguments {
        match argument.as_ref() {
            "--help" | "-h" => return Ok(FetchAction::ShowHelp),
            "--report" => {
                return Err("fetch expects the report as the first positional argument".to_string());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown fetch option {other:?}"));
            }
            other if report_id.is_none() => report_id = Some(other.to_string()),
            other => requested_families.push(other.to_string()),
        }
    }

    Ok(FetchAction::Fetch(FetchRequest {
        report_id,
        requested_families,
    }))
}

pub(super) fn resolve_fetch_options<'a>(
    request: FetchRequest,
    reports: &'a [Report],
) -> Result<FetchOptions<'a>, String> {
    let Some(report_id) = request.report_id else {
        return Err(missing_report_error(reports));
    };
    let report = report_for_id(&report_id, reports)?;
    Ok(FetchOptions {
        report,
        requested_families: request.requested_families,
    })
}

fn report_for_id<'a>(report_id: &str, reports: &'a [Report]) -> Result<&'a Report, String> {
    reports
        .iter()
        .find(|report| report.id == report_id)
        .ok_or_else(|| unknown_report_error(report_id, reports))
}

fn missing_report_error(reports: &[Report]) -> String {
    format!(
        "test ROM report must be provided; available reports: {}",
        available_reports(reports)
    )
}

fn unknown_report_error(report_id: &str, reports: &[Report]) -> String {
    format!(
        "unknown test ROM report {report_id:?}; available reports: {}",
        available_reports(reports)
    )
}

fn available_reports(reports: &[Report]) -> String {
    reports
        .iter()
        .map(|report| report.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
