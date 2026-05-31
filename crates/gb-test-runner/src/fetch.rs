mod cli;
mod git;
mod manifest;
mod materialize;
mod validate;

#[cfg(test)]
mod test;

use std::io::Write;
use std::path::Path;

use cli::{FetchAction, parse_fetch_arguments, resolve_fetch_options};
use git::{cleanup_fetched_sources, fetch_sources_into_temps};
use manifest::{
    filter_sources_for_families, load_report_manifest, load_source_manifest, report_families,
    select_families,
};
use materialize::{
    materialize_selected_sources, replace_selected_family_roots, store_root_for_report,
};
use validate::validate_materialization_targets;

const REPORTS_MANIFEST_PATH: &str = "crates/gb-test-runner/data/reports.toml";
const DATA_DIR: &str = "crates/gb-test-runner/data";

pub fn fetch_help_text() -> &'static str {
    concat!(
        "Usage: cargo run -p gb-test-runner --bin fetch -- <report-id> [family ...]\n",
        "\n",
        "Fetches pinned upstream ROM source(s) through the report registry, verifies SHA-256 hashes, materializes selected families under test/<report-store>, and removes temporary checkout(s).\n",
        "Report ids are read from crates/gb-test-runner/data/reports.toml.\n",
    )
}

pub fn run_fetch_command<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_fetch_arguments(arguments)? {
        FetchAction::ShowHelp => write_all(output, fetch_help_text()),
        FetchAction::Fetch(request) => {
            let reports = load_report_manifest(workspace_root)?;
            let options = resolve_fetch_options(request, &reports.reports)?;
            let report = options.report;
            let source_manifest = load_source_manifest(workspace_root, report)?;
            let available_families = report_families(report, &source_manifest)?;
            let selected_families =
                select_families(report, &available_families, &options.requested_families)?;
            let filtered_sources =
                filter_sources_for_families(&source_manifest.sources, report, &selected_families)?;
            validate_materialization_targets(report, &filtered_sources)?;

            let fetched_sources = fetch_sources_into_temps(&filtered_sources, output)?;
            let store_root = store_root_for_report(workspace_root, report);
            let result = (|| {
                replace_selected_family_roots(&store_root, report, &filtered_sources)?;
                materialize_selected_sources(&store_root, &fetched_sources)?;
                writeln_checked(
                    output,
                    &format!(
                        "materialized test ROM families {} into {}",
                        selected_families.join(", "),
                        store_root.display()
                    ),
                )?;
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
            }
        }
    }
}

fn write_all<W: Write>(output: &mut W, text: &str) -> Result<(), String> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write command output: {error}"))
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}").map_err(|error| format!("failed to write command output: {error}"))
}
