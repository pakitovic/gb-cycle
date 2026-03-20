use std::io::Write;
use std::path::PathBuf;

use crate::{
    CapturedArtifacts, RomRunner, RomSuite, RomSuiteReport, Timeout, built_in_rom_suite_by_name,
    built_in_rom_suites,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum RomSuiteCliAction {
    ShowHelp,
    ListSuites,
    Run(RomSuiteCliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RomSuiteCliOptions {
    suite_name: String,
    case_id: Option<String>,
    failure_artifact_root: Option<PathBuf>,
    timeout_override: Option<Timeout>,
}

pub fn rom_suite_cli_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo run -p gb-test-runner --bin run_rom_suite -- --list\n",
        "  cargo run -p gb-test-runner --bin run_rom_suite -- --suite <suite-name> [--case <case-id>] [--failure-artifact-root <dir>] [--timeout-frames <n> | --timeout-tcycles <n>]\n",
    )
}

pub fn run_rom_suite_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_rom_suite_command_with_runner(arguments, RomRunner::new(), output)
}

fn run_rom_suite_command_with_runner<I, S, W>(
    arguments: I,
    runner: RomRunner,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_rom_suite_arguments(arguments)? {
        RomSuiteCliAction::ShowHelp => write_all(output, rom_suite_cli_help_text()),
        RomSuiteCliAction::ListSuites => write_suite_catalog(output),
        RomSuiteCliAction::Run(options) => run_selected_suite(options, runner, output),
    }
}

fn parse_rom_suite_arguments<I, S>(arguments: I) -> Result<RomSuiteCliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut suite_name = None;
    let mut case_id = None;
    let mut failure_artifact_root = None;
    let mut timeout_override = None;

    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
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
            "--list" => return Ok(RomSuiteCliAction::ListSuites),
            "--help" | "-h" => return Ok(RomSuiteCliAction::ShowHelp),
            other => return Err(format!("unknown argument {other:?}; run with --help")),
        }
    }

    let Some(suite_name) = suite_name else {
        return Err("missing required --suite <suite-name>; run with --list".to_string());
    };

    Ok(RomSuiteCliAction::Run(RomSuiteCliOptions {
        suite_name,
        case_id,
        failure_artifact_root,
        timeout_override,
    }))
}

fn run_selected_suite<W: Write>(
    options: RomSuiteCliOptions,
    runner: RomRunner,
    output: &mut W,
) -> Result<(), String> {
    let mut suite = select_suite_for_options(&options)?;

    if let Some(timeout_override) = options.timeout_override {
        for case in &mut suite.cases {
            case.timeout = timeout_override;
        }
    }

    let mut runner = runner;
    if let Some(root) = options.failure_artifact_root {
        runner = runner.with_failure_artifact_root(root);
    }

    let report = runner
        .run_suite(&suite)
        .map_err(|error| format!("failed to execute suite {}: {error:?}", suite.name))?;
    write_suite_report(output, &report)?;

    if report.all_passed() {
        Ok(())
    } else {
        Err("one or more ROM cases failed".to_string())
    }
}

fn select_suite_for_options(options: &RomSuiteCliOptions) -> Result<RomSuite, String> {
    let Some(mut suite) = built_in_rom_suite_by_name(&options.suite_name) else {
        return Err(format!(
            "unknown suite {:?}; run with --list for the built-in catalog",
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

fn write_suite_catalog<W: Write>(output: &mut W) -> Result<(), String> {
    for suite in built_in_rom_suites() {
        writeln_checked(
            output,
            &format!("suite={} subsystem={:?}", suite.name, suite.subsystem),
        )?;
        for case in &suite.cases {
            writeln_checked(output, &format!("  case={}", case.id))?;
        }
    }

    Ok(())
}

fn write_suite_report<W: Write>(output: &mut W, report: &RomSuiteReport) -> Result<(), String> {
    writeln_checked(
        output,
        &format!(
            "suite={} subsystem={:?}",
            report.suite_name, report.subsystem
        ),
    )?;

    for case in &report.cases {
        writeln_checked(
            output,
            &format!(
                "case={} outcome={:?} t_cycles={} frames={}",
                case.case_id, case.outcome, case.executed_t_cycles, case.completed_frames
            ),
        )?;
        if !case.diagnostics.is_empty() {
            writeln_checked(output, &format!("diagnostics={:#?}", case.diagnostics))?;
        }
        write_artifacts(output, &case.artifacts)?;
        if !case.retained_failure_artifacts.is_empty() {
            writeln_checked(
                output,
                &format!(
                    "retained_failure_artifacts={:#?}",
                    case.retained_failure_artifacts
                ),
            )?;
        }
    }

    Ok(())
}

fn write_artifacts<W: Write>(output: &mut W, artifacts: &CapturedArtifacts) -> Result<(), String> {
    if let Some(serial) = &artifacts.serial {
        writeln_checked(output, &format!("serial=\n{serial}"))?;
    }

    if let Some(memory) = &artifacts.memory_text_output {
        writeln_checked(
            output,
            &format!(
                "memory_text_output=status=0x{status:02X} signature={signature:02X?}\n{text}",
                status = memory.status,
                signature = memory.signature,
                text = memory.text
            ),
        )?;
    }

    if let Some(console_text) = &artifacts.blargg_console_text {
        writeln_checked(output, &format!("blargg_console_text=\n{console_text}"))?;
    }

    if let Some(snapshot) = &artifacts.snapshot_text {
        writeln_checked(output, &format!("snapshot=\n{snapshot}"))?;
    }

    if let Some(trace) = &artifacts.trace {
        writeln_checked(output, &format!("trace=\n{trace}"))?;
    }

    if let Some(framebuffer) = &artifacts.framebuffer_pgm {
        writeln_checked(
            output,
            &format!("framebuffer_pgm_bytes={}", framebuffer.len()),
        )?;
    }

    Ok(())
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
    use std::path::PathBuf;

    use crate::{CapturedMemoryTextOutput, TestSubsystem};

    use super::{
        RomSuiteCliAction, RomSuiteCliOptions, parse_rom_suite_arguments, rom_suite_cli_help_text,
        run_rom_suite_command_with_runner, select_suite_for_options, write_suite_report,
    };
    use crate::{
        CapturedArtifacts, RomCaseOutcome, RomCaseReport, RomRunner, RomSuiteReport,
        default_workspace_root,
    };

    #[test]
    fn parse_arguments_supports_help_list_and_timeout_overrides() {
        assert_eq!(
            parse_rom_suite_arguments(["--help"]).expect("help should parse"),
            RomSuiteCliAction::ShowHelp
        );
        assert_eq!(
            parse_rom_suite_arguments(["--list"]).expect("list should parse"),
            RomSuiteCliAction::ListSuites
        );
        assert_eq!(
            parse_rom_suite_arguments([
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
            RomSuiteCliAction::Run(RomSuiteCliOptions {
                suite_name: "phase-2-cpu-timing".to_string(),
                case_id: Some("phase2-fetch-immediate-order".to_string()),
                failure_artifact_root: Some(PathBuf::from("/tmp/artifacts")),
                timeout_override: Some(crate::Timeout::TCycles(1234)),
            })
        );
    }

    #[test]
    fn parse_arguments_rejects_missing_and_invalid_values() {
        let missing_suite = parse_rom_suite_arguments(std::iter::empty::<&str>())
            .expect_err("missing suite should be rejected");
        assert!(missing_suite.contains("missing required --suite"));

        let invalid_timeout =
            parse_rom_suite_arguments(["--suite", "phase-2-cpu-timing", "--timeout-frames", "NaN"])
                .expect_err("invalid timeout should be rejected");
        assert!(invalid_timeout.contains("invalid --timeout-frames value"));
    }

    #[test]
    fn select_suite_rejects_unknown_suites_and_unknown_cases() {
        let unknown_suite = select_suite_for_options(&RomSuiteCliOptions {
            suite_name: "unknown".to_string(),
            case_id: None,
            failure_artifact_root: None,
            timeout_override: None,
        })
        .expect_err("unknown suite should be rejected");
        assert!(unknown_suite.contains("unknown suite"));

        let unknown_case = select_suite_for_options(&RomSuiteCliOptions {
            suite_name: "phase-2-cpu-timing".to_string(),
            case_id: Some("missing-case".to_string()),
            failure_artifact_root: None,
            timeout_override: None,
        })
        .expect_err("unknown case should be rejected");
        assert!(unknown_case.contains("does not contain case"));
    }

    #[test]
    fn list_and_help_commands_render_human_readable_output() {
        let mut help_output = Vec::new();
        run_rom_suite_command_with_runner(["--help"], RomRunner::new(), &mut help_output)
            .expect("help command should succeed");
        assert_eq!(
            String::from_utf8(help_output).expect("help output should be utf-8"),
            rom_suite_cli_help_text()
        );

        let mut list_output = Vec::new();
        run_rom_suite_command_with_runner(["--list"], RomRunner::new(), &mut list_output)
            .expect("list command should succeed");
        let list_output = String::from_utf8(list_output).expect("list output should be utf-8");
        assert!(list_output.contains("suite=phase-2-cpu-timing"));
        assert!(list_output.contains("case=phase2-fetch-immediate-order"));
    }

    #[test]
    fn report_writer_covers_all_artifact_channels() {
        let report = RomSuiteReport {
            suite_name: "synthetic".to_string(),
            subsystem: TestSubsystem::Cpu,
            cases: vec![RomCaseReport {
                case_id: "case-a".to_string(),
                outcome: RomCaseOutcome::Failed(crate::RomCaseFailure::TimeoutExceeded),
                executed_t_cycles: 64,
                completed_frames: 0,
                diagnostics: Vec::new(),
                artifacts: CapturedArtifacts {
                    serial: Some("serial-text".to_string()),
                    memory_text_output: Some(CapturedMemoryTextOutput {
                        status: 0,
                        signature: [0xDE, 0xB0, 0x61],
                        text: "Passed".to_string(),
                    }),
                    blargg_console_text: Some("console-text".to_string()),
                    framebuffer_pgm: Some(vec![0; 8]),
                    trace: Some("trace-text".to_string()),
                    snapshot_text: Some("snapshot-text".to_string()),
                },
                retained_failure_artifacts: vec![PathBuf::from("/tmp/trace.txt")],
            }],
        };

        let mut output = Vec::new();
        write_suite_report(&mut output, &report).expect("report writer should succeed");
        let output = String::from_utf8(output).expect("report output should be utf-8");

        assert!(output.contains("suite=synthetic subsystem=Cpu"));
        assert!(output.contains("case=case-a outcome=Failed"));
        assert!(output.contains("serial=\nserial-text"));
        assert!(output.contains("memory_text_output=status=0x00"));
        assert!(output.contains("blargg_console_text=\nconsole-text"));
        assert!(output.contains("snapshot=\nsnapshot-text"));
        assert!(output.contains("trace=\ntrace-text"));
        assert!(output.contains("framebuffer_pgm_bytes=8"));
        assert!(output.contains("retained_failure_artifacts="));
    }

    #[test]
    fn run_command_can_execute_a_known_built_in_case() {
        let mut output = Vec::new();
        run_rom_suite_command_with_runner(
            [
                "--suite",
                "phase-2-cpu-timing",
                "--case",
                "phase2-fetch-immediate-order",
            ],
            RomRunner::new().with_workspace_root(default_workspace_root()),
            &mut output,
        )
        .expect("built-in phase2 case should execute");

        let output = String::from_utf8(output).expect("command output should be utf-8");
        assert!(output.contains("suite=phase-2-cpu-timing"));
        assert!(output.contains("case=phase2-fetch-immediate-order outcome=Passed"));
    }
}
