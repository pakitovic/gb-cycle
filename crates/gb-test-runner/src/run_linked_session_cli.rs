use std::env;
use std::io::Write;
use std::path::PathBuf;

use gb_core::StartupMode;

use crate::{
    LinkedSessionCaseFailure, LinkedSessionCaseOutcome, LinkedSessionRunner, LinkedSessionSuite,
    LinkedSessionSuiteReport, built_in_linked_session_suite_by_name,
    load_linked_session_suite_manifest,
};

const TEST_ROM_STARTUP_ENV_VAR: &str = "GB_CYCLE_TEST_ROM_STARTUP";

#[derive(Debug, Clone, PartialEq, Eq)]
enum LinkedSessionCliAction {
    ShowHelp,
    Run(LinkedSessionCliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LinkedSessionCliTarget {
    BuiltIn { suite_name: String },
    Manifest { manifest_path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkedSessionCliOptions {
    target: LinkedSessionCliTarget,
    session_id: Option<String>,
    failure_artifact_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfiguredLinkedSessionStartup {
    Manifest,
    SkipBoot,
    CustomBoot,
    RealBoot,
}

pub fn linked_session_cli_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo run -p gb-test-runner --bin run_linked_session -- --suite <suite-name> [--session <session-id>] [--failure-artifact-root <dir>]\n",
        "  cargo run -p gb-test-runner --bin run_linked_session -- --manifest <path> [--session <session-id>] [--failure-artifact-root <dir>]\n",
    )
}

pub fn run_linked_session_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_linked_session_command_with_runner(arguments, LinkedSessionRunner::new(), output)
}

fn run_linked_session_command_with_runner<I, S, W>(
    arguments: I,
    runner: LinkedSessionRunner,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_linked_session_arguments(arguments)? {
        LinkedSessionCliAction::ShowHelp => write_all(output, linked_session_cli_help_text()),
        LinkedSessionCliAction::Run(options) => run_selected_suite(options, runner, output),
    }
}

fn parse_linked_session_arguments<I, S>(arguments: I) -> Result<LinkedSessionCliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut suite_name = None;
    let mut manifest_path = None;
    let mut session_id = None;
    let mut failure_artifact_root = None;

    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--suite" => {
                let Some(value) = arguments.next() else {
                    return Err("--suite requires a value".to_string());
                };
                suite_name = Some(value.as_ref().to_string());
            }
            "--manifest" => {
                let Some(value) = arguments.next() else {
                    return Err("--manifest requires a value".to_string());
                };
                manifest_path = Some(PathBuf::from(value.as_ref()));
            }
            "--session" => {
                let Some(value) = arguments.next() else {
                    return Err("--session requires a value".to_string());
                };
                session_id = Some(value.as_ref().to_string());
            }
            "--failure-artifact-root" => {
                let Some(value) = arguments.next() else {
                    return Err("--failure-artifact-root requires a value".to_string());
                };
                failure_artifact_root = Some(PathBuf::from(value.as_ref()));
            }
            "--help" | "-h" => return Ok(LinkedSessionCliAction::ShowHelp),
            other => return Err(format!("unknown argument {other:?}; run with --help")),
        }
    }

    let target = match (suite_name, manifest_path) {
        (Some(_), Some(_)) => {
            return Err(
                "--suite <suite-name> and --manifest <path> are mutually exclusive".to_string(),
            );
        }
        (Some(suite_name), None) => LinkedSessionCliTarget::BuiltIn { suite_name },
        (None, Some(manifest_path)) => LinkedSessionCliTarget::Manifest { manifest_path },
        (None, None) => {
            return Err(
                "missing required --suite <suite-name> or --manifest <path>; run with --help"
                    .to_string(),
            );
        }
    };

    Ok(LinkedSessionCliAction::Run(LinkedSessionCliOptions {
        target,
        session_id,
        failure_artifact_root,
    }))
}

fn run_selected_suite<W: Write>(
    options: LinkedSessionCliOptions,
    runner: LinkedSessionRunner,
    output: &mut W,
) -> Result<(), String> {
    let mut suite = select_suite_for_options(&options, &runner)?;
    apply_configured_startup_override(&mut suite)?;

    let mut runner = runner;
    if let Some(root) = options.failure_artifact_root {
        runner = runner.with_failure_artifact_root(root);
    }

    let report = runner
        .run_suite(&suite)
        .map_err(|error| format!("failed to execute linked suite {}: {error:?}", suite.name))?;
    write_suite_report(output, &report)?;

    if report.all_non_failing() {
        Ok(())
    } else {
        Err("one or more linked sessions failed".to_string())
    }
}

fn select_suite_for_options(
    options: &LinkedSessionCliOptions,
    runner: &LinkedSessionRunner,
) -> Result<LinkedSessionSuite, String> {
    let suite = match &options.target {
        LinkedSessionCliTarget::BuiltIn { suite_name } => {
            let Some(suite) =
                built_in_linked_session_suite_by_name(runner.workspace_root(), suite_name)
                    .map_err(|error| error.to_string())?
            else {
                return Err(format!("unknown linked suite {suite_name:?}"));
            };
            suite
        }
        LinkedSessionCliTarget::Manifest { manifest_path } => {
            load_linked_session_suite_manifest(manifest_path).map_err(|error| error.to_string())?
        }
    };

    select_session_for_options(suite, options.session_id.as_deref())
}

fn select_session_for_options(
    mut suite: LinkedSessionSuite,
    session_id: Option<&str>,
) -> Result<LinkedSessionSuite, String> {
    if let Some(session_id) = session_id {
        let Some(session) = suite
            .sessions
            .into_iter()
            .find(|session| session.id == session_id)
        else {
            return Err(format!(
                "suite {:?} does not contain session {:?}",
                suite.name, session_id
            ));
        };

        suite = if let Some(family) = suite.family {
            LinkedSessionSuite::new(suite.name)
                .with_family(family)
                .with_session(session)
        } else {
            LinkedSessionSuite::new(suite.name).with_session(session)
        };
    }

    Ok(suite)
}

fn configured_linked_session_startup() -> Result<ConfiguredLinkedSessionStartup, String> {
    configured_linked_session_startup_from_env_value(env::var(TEST_ROM_STARTUP_ENV_VAR))
}

fn configured_linked_session_startup_from_env_value(
    value: Result<String, env::VarError>,
) -> Result<ConfiguredLinkedSessionStartup, String> {
    match value {
        Ok(value) => match value.as_str() {
            "skip-boot" => Ok(ConfiguredLinkedSessionStartup::SkipBoot),
            "custom-boot" => Ok(ConfiguredLinkedSessionStartup::CustomBoot),
            "real-boot" => Ok(ConfiguredLinkedSessionStartup::RealBoot),
            other => Err(format!(
                "unsupported {TEST_ROM_STARTUP_ENV_VAR} value {other:?}; expected \"skip-boot\", \"custom-boot\", or \"real-boot\""
            )),
        },
        Err(env::VarError::NotPresent) => Ok(ConfiguredLinkedSessionStartup::Manifest),
        Err(env::VarError::NotUnicode(_)) => Err(format!(
            "{TEST_ROM_STARTUP_ENV_VAR} must be valid UTF-8; expected \"skip-boot\", \"custom-boot\", or \"real-boot\""
        )),
    }
}

fn apply_configured_startup_override(suite: &mut LinkedSessionSuite) -> Result<(), String> {
    apply_configured_startup_override_for(suite, configured_linked_session_startup()?)
}

fn apply_configured_startup_override_for(
    suite: &mut LinkedSessionSuite,
    startup: ConfiguredLinkedSessionStartup,
) -> Result<(), String> {
    match startup {
        ConfiguredLinkedSessionStartup::Manifest => {}
        ConfiguredLinkedSessionStartup::SkipBoot => {
            for session in &mut suite.sessions {
                for participant in &mut session.participants {
                    participant.startup_mode = StartupMode::SkipBoot;
                }
            }
        }
        ConfiguredLinkedSessionStartup::CustomBoot => {
            for session in &mut suite.sessions {
                for participant in &mut session.participants {
                    participant.startup_mode = StartupMode::CustomBoot;
                }
            }
        }
        ConfiguredLinkedSessionStartup::RealBoot => {
            for session in &mut suite.sessions {
                for participant in &mut session.participants {
                    participant.startup_mode = StartupMode::RealBoot;
                }
            }
        }
    }

    Ok(())
}

fn write_suite_report<W: Write>(
    output: &mut W,
    report: &LinkedSessionSuiteReport,
) -> Result<(), String> {
    writeln_checked(
        output,
        &format!(
            "suite={} family={}",
            report.suite_name,
            report.family.as_deref().unwrap_or("-"),
        ),
    )?;

    for session in &report.sessions {
        writeln_checked(
            output,
            &format!(
                "session={} outcome={} executed_t_cycles={}",
                session.session_id,
                outcome_label(&session.outcome),
                session.executed_t_cycles
            ),
        )?;
        if let LinkedSessionCaseOutcome::Failed(failure) = &session.outcome {
            writeln_checked(output, &format!("failure={}", render_failure(failure)))?;
        }
        for participant in &session.participants {
            writeln_checked(
                output,
                &format!(
                    "participant={} outcome={} completed_frames={} rom={} serial_hex={}",
                    participant.participant_id,
                    outcome_label(&participant.outcome),
                    participant.completed_frames,
                    participant.rom_path.display(),
                    participant.artifacts.serial_hex,
                ),
            )?;
        }
        for artifact in &session.retained_failure_artifacts {
            writeln_checked(output, &format!("failure_artifact={}", artifact.display()))?;
        }
    }

    Ok(())
}

fn outcome_label(outcome: &LinkedSessionCaseOutcome) -> &'static str {
    match outcome {
        LinkedSessionCaseOutcome::Passed => "PASS",
        LinkedSessionCaseOutcome::Informational => "INFO",
        LinkedSessionCaseOutcome::Failed(_) => "FAIL",
    }
}

fn render_failure(failure: &LinkedSessionCaseFailure) -> String {
    match failure {
        LinkedSessionCaseFailure::CpuDiagnosticTrap {
            participant_id,
            trap,
        } => format!(
            "cpu-diagnostic-trap participant={} trap={trap:?}",
            participant_id
        ),
        LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
            participant_id,
            expected,
            actual,
        } => format!(
            "participant-serial-hex-mismatch participant={} expected={} actual={}",
            participant_id, expected, actual
        ),
        LinkedSessionCaseFailure::ParticipantFixtureMismatch {
            participant_id,
            capture,
            fixture_path,
        } => format!(
            "participant-fixture-mismatch participant={} capture={:?} fixture={}",
            participant_id,
            capture,
            fixture_path.display()
        ),
        LinkedSessionCaseFailure::ParticipantFramebufferCheckAtNotReached {
            participant_id,
            check_at_tcycles,
            executed_t_cycles,
        } => format!(
            "participant-framebuffer-check-at-not-reached participant={} check_at_tcycles={} executed_t_cycles={}",
            participant_id, check_at_tcycles, executed_t_cycles
        ),
        LinkedSessionCaseFailure::FixtureMismatch { fixture_path } => {
            format!("fixture-mismatch fixture={}", fixture_path.display())
        }
    }
}

fn write_all<W: Write>(output: &mut W, text: &str) -> Result<(), String> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write CLI output: {error}"))
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    output
        .write_all(line.as_bytes())
        .and_then(|()| output.write_all(b"\n"))
        .map_err(|error| format!("failed to write CLI output: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        LinkedSessionCaptureKind, LinkedSessionCase, LinkedSessionParticipant,
        LinkedSessionPassCondition, Timeout,
    };
    use std::path::PathBuf;

    #[test]
    fn parse_help_action() {
        assert_eq!(
            parse_linked_session_arguments(["--help"]).expect("help should parse"),
            LinkedSessionCliAction::ShowHelp
        );
    }

    #[test]
    fn parse_requires_exactly_one_target() {
        let error = parse_linked_session_arguments([
            "--suite",
            "sample-linked-suite",
            "--manifest",
            "suite.toml",
        ])
        .expect_err("suite and manifest should be mutually exclusive");
        assert!(error.contains("mutually exclusive"));

        let missing = parse_linked_session_arguments(std::iter::empty::<&str>())
            .expect_err("missing target should fail");
        assert!(missing.contains("missing required --suite"));
    }

    #[test]
    fn configured_startup_override_rewrites_all_participants() {
        let session = LinkedSessionCase::new(
            "startup-override",
            crate::LinkedSessionTopology::Dmg04,
            Timeout::TCycles(1),
            LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace),
        )
        .with_participant(
            LinkedSessionParticipant::new("left", "left.gb")
                .with_startup_mode(StartupMode::SkipBoot),
        )
        .with_participant(
            LinkedSessionParticipant::new("right", "right.gb")
                .with_startup_mode(StartupMode::SkipBoot),
        );
        let mut suite = LinkedSessionSuite::new("startup-suite").with_session(session);

        apply_configured_startup_override_for(&mut suite, ConfiguredLinkedSessionStartup::RealBoot)
            .expect("startup override should succeed");

        assert!(
            suite.sessions[0]
                .participants
                .iter()
                .all(|participant| participant.startup_mode == StartupMode::RealBoot)
        );
    }

    #[test]
    fn configured_startup_env_parser_matches_rom_suite_values() {
        assert_eq!(
            configured_linked_session_startup_from_env_value(Ok("skip-boot".to_string()))
                .expect("skip boot should parse"),
            ConfiguredLinkedSessionStartup::SkipBoot
        );
        assert_eq!(
            configured_linked_session_startup_from_env_value(Ok("custom-boot".to_string()))
                .expect("custom boot should parse"),
            ConfiguredLinkedSessionStartup::CustomBoot
        );
        assert_eq!(
            configured_linked_session_startup_from_env_value(Ok("real-boot".to_string()))
                .expect("real boot should parse"),
            ConfiguredLinkedSessionStartup::RealBoot
        );
        assert!(
            configured_linked_session_startup_from_env_value(Ok("boot-rom".to_string()))
                .expect_err("unknown startup should fail")
                .contains("unsupported GB_CYCLE_TEST_ROM_STARTUP")
        );
    }

    #[test]
    fn render_failure_formats_participant_fixture_mismatches() {
        let rendered = render_failure(&LinkedSessionCaseFailure::ParticipantFixtureMismatch {
            participant_id: "left".to_string(),
            capture: crate::LinkedSessionCaptureKind::Snapshot,
            fixture_path: PathBuf::from("/tmp/left.snapshot"),
        });

        assert_eq!(
            rendered,
            "participant-fixture-mismatch participant=left capture=Snapshot fixture=/tmp/left.snapshot"
        );

        let rendered = render_failure(
            &LinkedSessionCaseFailure::ParticipantFramebufferCheckAtNotReached {
                participant_id: "left".to_string(),
                check_at_tcycles: 16,
                executed_t_cycles: 8,
            },
        );

        assert_eq!(
            rendered,
            "participant-framebuffer-check-at-not-reached participant=left check_at_tcycles=16 executed_t_cycles=8"
        );
    }
}
