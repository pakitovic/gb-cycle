use std::io::Write;

use crate::{
    DeterminismCaseFailure, DeterminismCaseOutcome, DeterminismRunner, DeterminismSuiteReport,
    RomSuite, built_in_rom_suite_by_name,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeterminismCliAction {
    ShowHelp,
    Run(DeterminismCliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeterminismCliOptions {
    suite_name: String,
    case_id: Option<String>,
    save_at_t_cycles: Option<u64>,
    continuation_t_cycles: Option<u64>,
}

pub fn determinism_cli_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo run -p gb-test-runner --bin run_determinism -- --suite <suite-name> [--case <case-id>] [--save-at-tcycles <n>] [--continuation-tcycles <n>]\n",
        "\n",
        "Runs Phase 9 deterministic replay plus in-memory save/load continuation checks.\n",
        "Only Strict cases count as passing closure evidence; non-Strict cases fail fast.\n",
        "When no window is specified, the runner uses a small deterministic window derived\n",
        "from the case timeout and capped at 1024 T-cycles on each side of the restore.\n",
    )
}

pub fn run_determinism_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_determinism_arguments(arguments)? {
        DeterminismCliAction::ShowHelp => write_all(output, determinism_cli_help_text()),
        DeterminismCliAction::Run(options) => run_selected_suite(options, output),
    }
}

fn parse_determinism_arguments<I, S>(arguments: I) -> Result<DeterminismCliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut suite_name = None;
    let mut case_id = None;
    let mut save_at_t_cycles = None;
    let mut continuation_t_cycles = None;

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
            "--save-at-tcycles" => {
                let Some(value) = arguments.next() else {
                    return Err("--save-at-tcycles requires a value".to_string());
                };
                save_at_t_cycles = Some(parse_positive_u64("--save-at-tcycles", value.as_ref())?);
            }
            "--continuation-tcycles" => {
                let Some(value) = arguments.next() else {
                    return Err("--continuation-tcycles requires a value".to_string());
                };
                continuation_t_cycles = Some(parse_positive_u64(
                    "--continuation-tcycles",
                    value.as_ref(),
                )?);
            }
            "--help" | "-h" => return Ok(DeterminismCliAction::ShowHelp),
            other => return Err(format!("unknown argument {other:?}; run with --help")),
        }
    }

    let Some(suite_name) = suite_name else {
        return Err(
            "missing required --suite <suite-name>; run run_rom_suite -- --list".to_string(),
        );
    };

    Ok(DeterminismCliAction::Run(DeterminismCliOptions {
        suite_name,
        case_id,
        save_at_t_cycles,
        continuation_t_cycles,
    }))
}

fn parse_positive_u64(name: &str, value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|error| format!("invalid {name} value {value:?}: {error}"))?;
    if parsed == 0 {
        return Err(format!(
            "invalid {name} value {value:?}: value must be non-zero"
        ));
    }
    Ok(parsed)
}

fn run_selected_suite<W: Write>(
    options: DeterminismCliOptions,
    output: &mut W,
) -> Result<(), String> {
    let suite = select_suite_for_options(&options)?;
    let mut runner = DeterminismRunner::new();
    if let Some(save_at_t_cycles) = options.save_at_t_cycles {
        runner = runner.with_save_at_t_cycles(save_at_t_cycles);
    }
    if let Some(continuation_t_cycles) = options.continuation_t_cycles {
        runner = runner.with_continuation_t_cycles(continuation_t_cycles);
    }

    let report = runner.run_suite(&suite).map_err(|error| {
        format!(
            "failed to execute determinism suite {}: {error:?}",
            suite.name
        )
    })?;
    write_suite_report(output, &report)?;

    if report.all_passed() {
        Ok(())
    } else {
        Err("one or more determinism cases failed".to_string())
    }
}

fn select_suite_for_options(options: &DeterminismCliOptions) -> Result<RomSuite, String> {
    let Some(mut suite) = built_in_rom_suite_by_name(&options.suite_name) else {
        return Err(format!(
            "unknown suite {:?}; run run_rom_suite -- --list for the built-in catalog",
            options.suite_name
        ));
    };

    if let Some(case_id) = &options.case_id {
        let Some(case) = suite.cases.into_iter().find(|case| &case.id == case_id) else {
            return Err(format!(
                "unknown case {case_id:?} in suite {:?}",
                options.suite_name
            ));
        };
        suite = if let Some(family) = suite.family {
            RomSuite::new(suite.name, suite.subsystem)
                .with_family(family)
                .with_case(case)
        } else {
            RomSuite::new(suite.name, suite.subsystem).with_case(case)
        };
    }

    Ok(suite)
}

fn write_suite_report<W: Write>(
    output: &mut W,
    report: &DeterminismSuiteReport,
) -> Result<(), String> {
    writeln_checked(
        output,
        &format!(
            "suite={} subsystem={:?} determinism=phase9",
            report.suite_name, report.subsystem
        ),
    )?;

    for case in &report.cases {
        writeln_checked(
            output,
            &format!(
                "case={} outcome={} save_at_t_cycles={} continuation_t_cycles={}",
                case.case_id,
                determinism_outcome_name(&case.outcome),
                case.save_at_t_cycles,
                case.continuation_t_cycles
            ),
        )?;
        if let DeterminismCaseOutcome::Failed(failure) = &case.outcome {
            writeln_checked(
                output,
                &format!("failure={}", determinism_failure_name(failure)),
            )?;
            if let DeterminismCaseFailure::ReplaySerialMismatch {
                baseline_hex,
                replay_hex,
            } = failure
            {
                writeln_checked(output, &format!("baseline_serial_hex={baseline_hex}"))?;
                writeln_checked(output, &format!("replay_serial_hex={replay_hex}"))?;
            }
            if let DeterminismCaseFailure::RestoreFailed { message } = failure {
                writeln_checked(output, &format!("restore_error={message}"))?;
            }
        }
    }

    Ok(())
}

fn determinism_outcome_name(outcome: &DeterminismCaseOutcome) -> &'static str {
    match outcome {
        DeterminismCaseOutcome::Passed => "passed",
        DeterminismCaseOutcome::Failed(_) => "failed",
    }
}

fn determinism_failure_name(failure: &DeterminismCaseFailure) -> &'static str {
    match failure {
        DeterminismCaseFailure::NonStrictExecutionMode { .. } => "non-strict-execution-mode",
        DeterminismCaseFailure::ReplayStateMismatch => "replay-state-mismatch",
        DeterminismCaseFailure::ReplaySerialMismatch { .. } => "replay-serial-mismatch",
        DeterminismCaseFailure::SaveLoadStateMismatch => "save-load-state-mismatch",
        DeterminismCaseFailure::MetadataGuardAcceptedMismatchedState => {
            "metadata-guard-accepted-mismatched-state"
        }
        DeterminismCaseFailure::RestoreFailed { .. } => "restore-failed",
        DeterminismCaseFailure::CpuDiagnosticTrap { .. } => "cpu-diagnostic-trap",
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
    use std::io::{self, Write};

    use crate::{
        DeterminismCaseFailure, DeterminismCaseOutcome, DeterminismCaseReport,
        DeterminismSuiteReport, TestSubsystem, determinism_cli_help_text, run_determinism_command,
    };

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn help_mentions_strict_determinism_scope() {
        assert!(determinism_cli_help_text().contains("run_determinism"));
        assert!(determinism_cli_help_text().contains("Only Strict cases"));
    }

    #[test]
    fn parse_supports_case_and_window_overrides() {
        assert_eq!(
            super::parse_determinism_arguments([
                "--suite",
                "phase-2-cpu-timing",
                "--case",
                "phase2-fetch-immediate-order",
                "--save-at-tcycles",
                "7",
                "--continuation-tcycles",
                "9",
            ])
            .expect("determinism args should parse"),
            super::DeterminismCliAction::Run(super::DeterminismCliOptions {
                suite_name: "phase-2-cpu-timing".to_string(),
                case_id: Some("phase2-fetch-immediate-order".to_string()),
                save_at_t_cycles: Some(7),
                continuation_t_cycles: Some(9),
            })
        );
    }

    #[test]
    fn parse_errors_cover_missing_and_invalid_windows() {
        let missing = super::parse_determinism_arguments(["--suite"])
            .expect_err("missing suite value should fail");
        assert!(missing.contains("--suite requires a value"));

        let invalid = super::parse_determinism_arguments([
            "--suite",
            "phase-2-cpu-timing",
            "--save-at-tcycles",
            "0",
        ])
        .expect_err("zero save window should fail");
        assert!(invalid.contains("non-zero"));

        let missing_case =
            super::parse_determinism_arguments(["--suite", "phase-2-cpu-timing", "--case"])
                .expect_err("missing case value should fail");
        assert!(missing_case.contains("--case requires a value"));

        let invalid_continuation = super::parse_determinism_arguments([
            "--suite",
            "phase-2-cpu-timing",
            "--continuation-tcycles",
            "NaN",
        ])
        .expect_err("invalid continuation window should fail");
        assert!(invalid_continuation.contains("invalid --continuation-tcycles"));

        let unknown =
            super::parse_determinism_arguments(["--suite", "phase-2-cpu-timing", "--unknown"])
                .expect_err("unknown argument should fail");
        assert!(unknown.contains("unknown argument"));
    }

    #[test]
    fn help_command_surfaces_writer_failures() {
        let error = run_determinism_command(["--help"], &mut FailingWriter)
            .expect_err("broken help writer should fail");
        assert!(error.contains("failed to write command output"));
    }

    #[test]
    fn command_executes_known_phase2_case() {
        let mut output = Vec::new();
        run_determinism_command(
            [
                "--suite",
                "phase-2-cpu-timing",
                "--case",
                "phase2-fetch-immediate-order",
            ],
            &mut output,
        )
        .expect("phase2 determinism case should pass");

        let output = String::from_utf8(output).expect("output should be utf-8");
        assert!(output.contains("suite=phase-2-cpu-timing"));
        assert!(output.contains("outcome=passed"));
    }

    #[test]
    fn suite_report_failure_sets_nonzero_command_result() {
        let report = DeterminismSuiteReport {
            suite_name: "x".to_string(),
            subsystem: TestSubsystem::Cpu,
            cases: vec![DeterminismCaseReport {
                case_id: "case".to_string(),
                outcome: DeterminismCaseOutcome::Failed(
                    crate::DeterminismCaseFailure::ReplayStateMismatch,
                ),
                save_at_t_cycles: 1,
                continuation_t_cycles: 1,
            }],
        };

        let mut output = Vec::new();
        super::write_suite_report(&mut output, &report).expect("report should render");
        let output = String::from_utf8(output).expect("output should be utf-8");
        assert!(output.contains("failure=replay-state-mismatch"));
    }

    #[test]
    fn suite_report_renders_serial_and_restore_context_and_writer_errors() {
        let report = DeterminismSuiteReport {
            suite_name: "x".to_string(),
            subsystem: TestSubsystem::Serial,
            cases: vec![
                DeterminismCaseReport {
                    case_id: "serial".to_string(),
                    outcome: DeterminismCaseOutcome::Failed(
                        DeterminismCaseFailure::ReplaySerialMismatch {
                            baseline_hex: "AA".to_string(),
                            replay_hex: "BB".to_string(),
                        },
                    ),
                    save_at_t_cycles: 1,
                    continuation_t_cycles: 2,
                },
                DeterminismCaseReport {
                    case_id: "restore".to_string(),
                    outcome: DeterminismCaseOutcome::Failed(
                        DeterminismCaseFailure::RestoreFailed {
                            message: "metadata mismatch".to_string(),
                        },
                    ),
                    save_at_t_cycles: 3,
                    continuation_t_cycles: 4,
                },
                DeterminismCaseReport {
                    case_id: "metadata".to_string(),
                    outcome: DeterminismCaseOutcome::Failed(
                        DeterminismCaseFailure::MetadataGuardAcceptedMismatchedState,
                    ),
                    save_at_t_cycles: 5,
                    continuation_t_cycles: 6,
                },
            ],
        };

        let mut output = Vec::new();
        super::write_suite_report(&mut output, &report).expect("report should render");
        let output = String::from_utf8(output).expect("output should be utf-8");
        assert!(output.contains("suite=x subsystem=Serial determinism=phase9"));
        assert!(output.contains("failure=replay-serial-mismatch"));
        assert!(output.contains("baseline_serial_hex=AA"));
        assert!(output.contains("replay_serial_hex=BB"));
        assert!(output.contains("failure=restore-failed"));
        assert!(output.contains("restore_error=metadata mismatch"));
        assert!(output.contains("failure=metadata-guard-accepted-mismatched-state"));

        let error = super::write_suite_report(&mut FailingWriter, &report)
            .expect_err("broken report writer should fail");
        assert!(error.contains("failed to write command output"));
    }

    #[test]
    fn unknown_case_reports_suite_context() {
        let error = super::parse_determinism_arguments([
            "--suite",
            "phase-2-cpu-timing",
            "--case",
            "missing",
        ])
        .and_then(|action| match action {
            super::DeterminismCliAction::Run(options) => super::select_suite_for_options(&options),
            super::DeterminismCliAction::ShowHelp => unreachable!("not help"),
        })
        .expect_err("unknown case should fail");
        assert!(error.contains("unknown case"));
    }
}
