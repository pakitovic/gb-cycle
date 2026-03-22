use std::io::Write;
use std::path::PathBuf;

use crate::{
    DifferentialCaseOutcome, DifferentialOracle, DifferentialOracleLayout, DifferentialRunner,
    DifferentialSuiteReport, RomRunner, RomSuite, Timeout, built_in_rom_suite_by_name,
    default_workspace_root, oracle_layout_root,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum DifferentialCliAction {
    ShowHelp,
    Run(DifferentialCliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DifferentialCliOptions {
    oracle: DifferentialOracle,
    oracle_layout: DifferentialOracleLayout,
    oracle_artifact_root: PathBuf,
    suite_name: String,
    case_id: Option<String>,
    failure_artifact_root: Option<PathBuf>,
    timeout_override: Option<Timeout>,
}

pub fn differential_cli_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy [--oracle-layout <case-bundle|sameboy-tester>] --suite <suite-name> [--case <case-id>] [--oracle-artifact-root <dir>] [--failure-artifact-root <dir>] [--timeout-frames <n> | --timeout-tcycles <n>]\n",
        "\n",
        "The default oracle layout is case-bundle.\n",
        "If --oracle-artifact-root is omitted, the default repo-local root is\n",
        ".oracles/<oracle>/<layout>/.\n",
        "Case-bundle roots must contain one subdirectory per case id using the same\n",
        "artifact filenames that gb-test-runner already emits locally, such as serial.txt,\n",
        "memory_text_output.txt, blargg_console.txt, framebuffer.pgm, or trace.txt.\n",
        "The sameboy-tester layout is framebuffer-only and expects SameBoy Tester BMP/TGA\n",
        "artifacts located alongside ROM-relative paths, such as testroms/acid/dmg-acid2.bmp.\n",
    )
}

pub fn run_differential_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_differential_arguments(arguments)? {
        DifferentialCliAction::ShowHelp => write_all(output, differential_cli_help_text()),
        DifferentialCliAction::Run(options) => run_selected_suite(options, output),
    }
}

fn parse_differential_arguments<I, S>(arguments: I) -> Result<DifferentialCliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut oracle = None;
    let mut oracle_layout = DifferentialOracleLayout::CaseBundle;
    let mut oracle_artifact_root = None;
    let mut suite_name = None;
    let mut case_id = None;
    let mut failure_artifact_root = None;
    let mut timeout_override = None;

    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--oracle" => {
                let Some(value) = arguments.next() else {
                    return Err("--oracle requires a value".to_string());
                };
                oracle = Some(parse_oracle(value.as_ref())?);
            }
            "--oracle-layout" => {
                let Some(value) = arguments.next() else {
                    return Err("--oracle-layout requires a value".to_string());
                };
                oracle_layout = parse_oracle_layout(value.as_ref())?;
            }
            "--oracle-artifact-root" => {
                let Some(value) = arguments.next() else {
                    return Err("--oracle-artifact-root requires a value".to_string());
                };
                oracle_artifact_root = Some(PathBuf::from(value.as_ref()));
            }
            "--suite" => {
                let Some(value) = arguments.next() else {
                    return Err("--suite requires a value".to_string());
                };
                suite_name = Some(value.as_ref().to_string());
            }
            "--case" => {
                let Some(value) = arguments.next() else {
                    return Err("--case requires a value".to_string());
                };
                case_id = Some(value.as_ref().to_string());
            }
            "--failure-artifact-root" => {
                let Some(value) = arguments.next() else {
                    return Err("--failure-artifact-root requires a value".to_string());
                };
                failure_artifact_root = Some(PathBuf::from(value.as_ref()));
            }
            "--timeout-frames" => {
                let Some(value) = arguments.next() else {
                    return Err("--timeout-frames requires a value".to_string());
                };
                let parsed = value.as_ref().parse::<u32>().map_err(|error| {
                    format!(
                        "invalid --timeout-frames value {:?}: {error}",
                        value.as_ref()
                    )
                })?;
                timeout_override = Some(Timeout::Frames(parsed));
            }
            "--timeout-tcycles" => {
                let Some(value) = arguments.next() else {
                    return Err("--timeout-tcycles requires a value".to_string());
                };
                let parsed = value.as_ref().parse::<u64>().map_err(|error| {
                    format!(
                        "invalid --timeout-tcycles value {:?}: {error}",
                        value.as_ref()
                    )
                })?;
                timeout_override = Some(Timeout::TCycles(parsed));
            }
            "--help" | "-h" => return Ok(DifferentialCliAction::ShowHelp),
            other => return Err(format!("unknown argument {other:?}; run with --help")),
        }
    }

    let Some(oracle) = oracle else {
        return Err("missing required --oracle sameboy".to_string());
    };
    let Some(suite_name) = suite_name else {
        return Err(
            "missing required --suite <suite-name>; run run_rom_suite -- --list".to_string(),
        );
    };

    Ok(DifferentialCliAction::Run(DifferentialCliOptions {
        oracle,
        oracle_layout,
        oracle_artifact_root: oracle_artifact_root.unwrap_or_else(|| {
            oracle_layout_root(
                &default_workspace_root(),
                oracle.name(),
                oracle_layout.name(),
            )
        }),
        suite_name,
        case_id,
        failure_artifact_root,
        timeout_override,
    }))
}

fn parse_oracle(value: &str) -> Result<DifferentialOracle, String> {
    match value {
        "sameboy" => Ok(DifferentialOracle::SameBoy),
        _ => Err(format!("unknown oracle {value:?}; expected sameboy")),
    }
}

fn parse_oracle_layout(value: &str) -> Result<DifferentialOracleLayout, String> {
    match value {
        "case-bundle" => Ok(DifferentialOracleLayout::CaseBundle),
        "sameboy-tester" => Ok(DifferentialOracleLayout::SameBoyTester),
        _ => Err(format!(
            "unknown oracle layout {value:?}; expected case-bundle or sameboy-tester"
        )),
    }
}

fn run_selected_suite<W: Write>(
    options: DifferentialCliOptions,
    output: &mut W,
) -> Result<(), String> {
    let mut suite = select_suite_for_options(&options)?;

    if let Some(timeout_override) = options.timeout_override {
        for case in &mut suite.cases {
            case.timeout = timeout_override;
        }
    }

    let mut runner = DifferentialRunner::new(options.oracle, options.oracle_artifact_root)
        .with_oracle_layout(options.oracle_layout)
        .with_rom_runner(RomRunner::new());
    if let Some(root) = options.failure_artifact_root {
        runner = runner.with_failure_artifact_root(root);
    }

    let report = runner.run_suite(&suite).map_err(|error| {
        format!(
            "failed to execute differential suite {}: {error:?}",
            suite.name
        )
    })?;
    write_suite_report(output, &report)?;

    if report.all_matched() && report.cases.iter().all(|case| case.local_report.passed()) {
        Ok(())
    } else {
        Err("one or more differential cases diverged or failed local pass conditions".to_string())
    }
}

fn select_suite_for_options(options: &DifferentialCliOptions) -> Result<RomSuite, String> {
    let Some(mut suite) = built_in_rom_suite_by_name(&options.suite_name) else {
        return Err(format!(
            "unknown suite {:?}; run run_rom_suite -- --list for the built-in catalog",
            options.suite_name
        ));
    };

    if let Some(case_id) = &options.case_id {
        let Some(case) = suite.cases.into_iter().find(|case| case.id == *case_id) else {
            return Err(format!(
                "suite {:?} does not contain case {:?}",
                options.suite_name, case_id
            ));
        };
        suite = RomSuite::new(suite.name, suite.subsystem).with_case(case);
    }

    Ok(suite)
}

fn write_suite_report<W: Write>(
    output: &mut W,
    report: &DifferentialSuiteReport,
) -> Result<(), String> {
    writeln_checked(
        output,
        &format!(
            "suite={} subsystem={:?} oracle={} oracle_layout={}",
            report.suite_name,
            report.subsystem,
            report.oracle.name(),
            report.oracle_layout.name(),
        ),
    )?;

    for case in &report.cases {
        writeln_checked(
            output,
            &format!(
                "case={} local_outcome={:?} differential_outcome={} channel={} t_cycles={} frames={} oracle_artifact={}",
                case.case_id,
                case.local_report.outcome,
                differential_outcome_name(&case.outcome),
                capture_name(case.compared_capture),
                case.local_report.executed_t_cycles,
                case.local_report.completed_frames,
                case.oracle_artifact_path.display(),
            ),
        )?;
        if let DifferentialCaseOutcome::Diverged(mismatch) = &case.outcome {
            writeln_checked(output, &format!("mismatch={}", mismatch.name()))?;
            writeln_checked(output, &format!("mismatch_detail={}", mismatch.detail()))?;
        }
        if !case.archived_context_artifacts.is_empty() {
            writeln_checked(
                output,
                &format!(
                    "archived_context_artifacts={:#?}",
                    case.archived_context_artifacts
                ),
            )?;
        }
    }

    Ok(())
}

fn differential_outcome_name(outcome: &DifferentialCaseOutcome) -> &'static str {
    match outcome {
        DifferentialCaseOutcome::Matched => "matched",
        DifferentialCaseOutcome::Diverged(_) => "diverged",
    }
}

fn capture_name(capture: crate::CaptureKind) -> &'static str {
    match capture {
        crate::CaptureKind::Serial => "serial",
        crate::CaptureKind::MemoryTextOutput => "memory-text-output",
        crate::CaptureKind::BlarggConsoleText => "blargg-console-text",
        crate::CaptureKind::Framebuffer => "framebuffer",
        crate::CaptureKind::Trace => "trace",
        crate::CaptureKind::Snapshot => "snapshot",
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

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::artifact_file_name;

    use super::{
        DifferentialCliAction, DifferentialCliOptions, default_workspace_root, oracle_layout_root,
        parse_differential_arguments, run_differential_command, select_suite_for_options,
    };

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-differential-cli-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn parse_arguments_supports_help_and_run_options() {
        assert_eq!(
            parse_differential_arguments(["--help"]).expect("help should parse"),
            DifferentialCliAction::ShowHelp
        );
        assert_eq!(
            parse_differential_arguments([
                "--oracle",
                "sameboy",
                "--suite",
                "phase-2-cpu-timing",
                "--case",
                "phase2-fetch-immediate-order",
                "--failure-artifact-root",
                "/tmp/artifacts",
                "--timeout-tcycles",
                "1234",
            ])
            .expect("run args should parse"),
            DifferentialCliAction::Run(DifferentialCliOptions {
                oracle: crate::DifferentialOracle::SameBoy,
                oracle_layout: crate::DifferentialOracleLayout::CaseBundle,
                oracle_artifact_root: oracle_layout_root(
                    &default_workspace_root(),
                    "sameboy",
                    "case-bundle",
                ),
                suite_name: "phase-2-cpu-timing".to_string(),
                case_id: Some("phase2-fetch-immediate-order".to_string()),
                failure_artifact_root: Some(PathBuf::from("/tmp/artifacts")),
                timeout_override: Some(crate::Timeout::TCycles(1234)),
            })
        );
    }

    #[test]
    fn parse_arguments_rejects_missing_required_flags() {
        let missing_oracle = parse_differential_arguments(["--suite", "phase-2-cpu-timing"])
            .expect_err("missing oracle should be rejected");
        assert!(missing_oracle.contains("missing required --oracle"));

        let bad_oracle =
            parse_differential_arguments(["--oracle", "unknown", "--suite", "phase-2-cpu-timing"])
                .expect_err("unknown oracle should be rejected");
        assert!(bad_oracle.contains("unknown oracle"));

        let bad_layout = parse_differential_arguments([
            "--oracle",
            "sameboy",
            "--oracle-layout",
            "unknown",
            "--suite",
            "phase-2-cpu-timing",
        ])
        .expect_err("unknown layout should be rejected");
        assert!(bad_layout.contains("unknown oracle layout"));

        let default_root =
            parse_differential_arguments(["--oracle", "sameboy", "--suite", "phase-2-cpu-timing"])
                .expect("default oracle root should be inferred");
        let DifferentialCliAction::Run(default_root) = default_root else {
            panic!("expected run action");
        };
        assert_eq!(
            default_root.oracle_artifact_root,
            oracle_layout_root(&default_workspace_root(), "sameboy", "case-bundle")
        );
    }

    #[test]
    fn select_suite_rejects_unknown_suite_or_case() {
        let unknown_suite = select_suite_for_options(&DifferentialCliOptions {
            oracle: crate::DifferentialOracle::SameBoy,
            oracle_layout: crate::DifferentialOracleLayout::CaseBundle,
            oracle_artifact_root: PathBuf::from("/tmp/oracle"),
            suite_name: "unknown".to_string(),
            case_id: None,
            failure_artifact_root: None,
            timeout_override: None,
        })
        .expect_err("unknown suite should fail");
        assert!(unknown_suite.contains("unknown suite"));

        let unknown_case = select_suite_for_options(&DifferentialCliOptions {
            oracle: crate::DifferentialOracle::SameBoy,
            oracle_layout: crate::DifferentialOracleLayout::CaseBundle,
            oracle_artifact_root: PathBuf::from("/tmp/oracle"),
            suite_name: "phase-2-cpu-timing".to_string(),
            case_id: Some("missing".to_string()),
            failure_artifact_root: None,
            timeout_override: None,
        })
        .expect_err("unknown case should fail");
        assert!(unknown_case.contains("does not contain case"));
    }

    #[test]
    fn run_command_can_compare_a_known_phase_2_case() {
        let oracle_root = unique_temp_dir("phase2-match");
        let case_dir = oracle_root.join("phase2-fetch-immediate-order");
        fs::create_dir_all(&case_dir).expect("case dir should be creatable");
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../gb-core/tests/fixtures/traces/phase2/phase2_fetch_immediate_order.trace"),
            case_dir.join(artifact_file_name(crate::CaptureKind::Trace)),
        )
        .expect("oracle trace should be writable");

        let mut output = Vec::new();
        run_differential_command(
            [
                "--oracle",
                "sameboy",
                "--oracle-artifact-root",
                oracle_root.to_str().expect("temp path should be utf-8"),
                "--suite",
                "phase-2-cpu-timing",
                "--case",
                "phase2-fetch-immediate-order",
            ],
            &mut output,
        )
        .expect("differential command should succeed");

        let output = String::from_utf8(output).expect("command output should be utf-8");
        assert!(output.contains("suite=phase-2-cpu-timing subsystem=Cpu oracle=sameboy"));
        assert!(output.contains("case=phase2-fetch-immediate-order"));
        assert!(output.contains("differential_outcome=matched"));
    }

    #[test]
    fn run_command_reports_mismatch_detail_for_trace_case() {
        let oracle_root = unique_temp_dir("phase2-mismatch");
        let case_dir = oracle_root.join("phase2-fetch-immediate-order");
        fs::create_dir_all(&case_dir).expect("case dir should be creatable");
        fs::write(
            case_dir.join(artifact_file_name(crate::CaptureKind::Trace)),
            "wrong trace\n",
        )
        .expect("oracle trace should be writable");

        let mut output = Vec::new();
        let error = run_differential_command(
            [
                "--oracle",
                "sameboy",
                "--oracle-artifact-root",
                oracle_root.to_str().expect("temp path should be utf-8"),
                "--suite",
                "phase-2-cpu-timing",
                "--case",
                "phase2-fetch-immediate-order",
            ],
            &mut output,
        )
        .expect_err("mismatch should fail the command");
        assert!(error.contains("diverged"));

        let output = String::from_utf8(output).expect("command output should be utf-8");
        assert!(output.contains("mismatch=trace-mismatch"));
        assert!(output.contains("mismatch_detail=first_difference_byte="));
    }
}
