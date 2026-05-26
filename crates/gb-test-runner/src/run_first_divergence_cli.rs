use std::io::Write;
use std::path::PathBuf;

use crate::{
    FirstDivergenceCaseOutcome, FirstDivergenceCompareMode, FirstDivergenceRunner, RomRunner,
    RomSuite, SameBoyCaseBundleRunner, Timeout, built_in_rom_suite_by_name, default_workspace_root,
    oracle_layout_root,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum FirstDivergenceCliAction {
    ShowHelp,
    Run(FirstDivergenceCliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FirstDivergenceCliOptions {
    probe_root: PathBuf,
    sameboy_root: Option<PathBuf>,
    runner_binary: Option<PathBuf>,
    suite_name: String,
    case_id: Option<String>,
    timeout_override: Option<Timeout>,
    build_if_missing: bool,
    allow_divergence: bool,
    compare_mode: FirstDivergenceCompareMode,
    probe_interval_tcycles: u64,
}

pub fn first_divergence_cli_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo run -p gb-test-runner --bin run_first_divergence -- --oracle sameboy --suite <suite-name> [--case <case-id>] [--probe-root <dir>] [--sameboy-root <dir> | --runner-binary <path>] [--timeout-frames <n> | --timeout-tcycles <n>] [--probe-interval-tcycles <n>] [--compare-mode <framebuffer|state>] [--allow-divergence] [--build-if-missing]\n",
        "\n",
        "The runner captures periodic local and LibSameBoy probe JSONL files and reports the first window where the selected comparison mode diverges. The default compare mode is framebuffer, which compares normalized framebuffer hashes while preserving CPU/PPU/timer/memory hashes as context. If --probe-root is omitted, the default repo-local root is .oracles/sameboy/first-divergence/.\n",
        "\n",
        "Environment fallbacks:\n",
        "  GB_CYCLE_SAMEBOY_ROOT for --sameboy-root\n",
        "  GB_CYCLE_SAMEBOY_CASE_BUNDLE_BIN for --runner-binary\n",
    )
}

pub fn run_first_divergence_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_first_divergence_arguments(arguments)? {
        FirstDivergenceCliAction::ShowHelp => write_all(output, first_divergence_cli_help_text()),
        FirstDivergenceCliAction::Run(options) => run_selected_suite(options, output),
    }
}

fn parse_first_divergence_arguments<I, S>(arguments: I) -> Result<FirstDivergenceCliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut oracle_seen = false;
    let mut probe_root = None;
    let mut sameboy_root = None;
    let mut runner_binary = None;
    let mut suite_name = None;
    let mut case_id = None;
    let mut timeout_override = None;
    let mut build_if_missing = false;
    let mut allow_divergence = false;
    let mut compare_mode = FirstDivergenceCompareMode::Framebuffer;
    let mut probe_interval_tcycles = 70_224_u64;

    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--oracle" => {
                let Some(value) = arguments.next() else {
                    return Err("--oracle requires a value".to_string());
                };
                if value.as_ref() != "sameboy" {
                    return Err(format!(
                        "unknown oracle {:?}; expected sameboy",
                        value.as_ref()
                    ));
                }
                oracle_seen = true;
            }
            "--probe-root" => {
                let Some(value) = arguments.next() else {
                    return Err("--probe-root requires a value".to_string());
                };
                probe_root = Some(PathBuf::from(value.as_ref()));
            }
            "--sameboy-root" => {
                let Some(value) = arguments.next() else {
                    return Err("--sameboy-root requires a value".to_string());
                };
                sameboy_root = Some(PathBuf::from(value.as_ref()));
            }
            "--runner-binary" => {
                let Some(value) = arguments.next() else {
                    return Err("--runner-binary requires a value".to_string());
                };
                runner_binary = Some(PathBuf::from(value.as_ref()));
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
            "--probe-interval-tcycles" => {
                let Some(value) = arguments.next() else {
                    return Err("--probe-interval-tcycles requires a value".to_string());
                };
                probe_interval_tcycles = value.as_ref().parse::<u64>().map_err(|error| {
                    format!(
                        "invalid --probe-interval-tcycles value {:?}: {error}",
                        value.as_ref()
                    )
                })?;
                if probe_interval_tcycles == 0 {
                    return Err("--probe-interval-tcycles must be greater than zero".to_string());
                }
            }
            "--compare-mode" => {
                let Some(value) = arguments.next() else {
                    return Err("--compare-mode requires a value".to_string());
                };
                compare_mode = parse_compare_mode(value.as_ref())?;
            }
            "--build-if-missing" => build_if_missing = true,
            "--allow-divergence" => allow_divergence = true,
            "--help" | "-h" => return Ok(FirstDivergenceCliAction::ShowHelp),
            other => return Err(format!("unknown argument {other:?}; run with --help")),
        }
    }

    if !oracle_seen {
        return Err("missing required --oracle sameboy".to_string());
    }
    let Some(suite_name) = suite_name else {
        return Err(
            "missing required --suite <suite-name>; run run_rom_suite -- --list".to_string(),
        );
    };

    Ok(FirstDivergenceCliAction::Run(FirstDivergenceCliOptions {
        probe_root: probe_root.unwrap_or_else(|| {
            oracle_layout_root(&default_workspace_root(), "sameboy", "first-divergence")
        }),
        sameboy_root,
        runner_binary,
        suite_name,
        case_id,
        timeout_override,
        build_if_missing,
        allow_divergence,
        compare_mode,
        probe_interval_tcycles,
    }))
}

fn parse_compare_mode(value: &str) -> Result<FirstDivergenceCompareMode, String> {
    match value {
        "framebuffer" => Ok(FirstDivergenceCompareMode::Framebuffer),
        "state" => Ok(FirstDivergenceCompareMode::State),
        _ => Err(format!(
            "unknown compare mode {value:?}; expected framebuffer or state"
        )),
    }
}

fn run_selected_suite<W: Write>(
    options: FirstDivergenceCliOptions,
    output: &mut W,
) -> Result<(), String> {
    let mut suite = select_suite_for_options(&options)?;
    if let Some(timeout_override) = options.timeout_override {
        for case in &mut suite.cases {
            case.timeout = timeout_override;
        }
    }

    let rom_runner = RomRunner::new();
    let mut sameboy_runner = SameBoyCaseBundleRunner::new(&options.probe_root)
        .with_rom_runner(rom_runner.clone())
        .with_build_if_missing(options.build_if_missing);
    if let Some(sameboy_root) = options.sameboy_root {
        sameboy_runner = sameboy_runner.with_sameboy_root(sameboy_root);
    }
    if let Some(runner_binary) = options.runner_binary {
        sameboy_runner = sameboy_runner.with_runner_binary(runner_binary);
    }

    let runner = FirstDivergenceRunner::new(&options.probe_root)
        .with_rom_runner(rom_runner)
        .with_sameboy_runner(sameboy_runner)
        .with_compare_mode(options.compare_mode)
        .with_probe_interval_tcycles(options.probe_interval_tcycles);
    let report = runner.run_suite(&suite).map_err(|error| {
        format!(
            "failed to execute first-divergence suite {}: {error:?}",
            suite.name
        )
    })?;
    write_suite_report(output, &report)?;
    if report.all_matched() || options.allow_divergence {
        Ok(())
    } else {
        Err("one or more first-divergence cases diverged".to_string())
    }
}

fn select_suite_for_options(options: &FirstDivergenceCliOptions) -> Result<RomSuite, String> {
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
    report: &crate::FirstDivergenceSuiteReport,
) -> Result<(), String> {
    writeln_checked(
        output,
        &format!(
            "suite={} compare_mode={} probe_interval_tcycles={} probe_root={}",
            report.suite_name,
            report.compare_mode.name(),
            report.probe_interval_tcycles,
            report.probe_root.display(),
        ),
    )?;

    for case in &report.cases {
        writeln_checked(
            output,
            &format!(
                "case={} outcome={} local_probes={} oracle_probes={} local_probe_path={} oracle_probe_path={}",
                case.case_id,
                first_divergence_outcome_name(&case.outcome),
                case.local_probe_count,
                case.oracle_probe_count,
                case.local_probe_path.display(),
                case.oracle_probe_path.display(),
            ),
        )?;
        if let FirstDivergenceCaseOutcome::Diverged {
            first_probe_index,
            window_start_tcycles,
            local_tcycles,
            oracle_tcycles,
            mismatches,
        } = &case.outcome
        {
            writeln_checked(
                output,
                &format!(
                    "first_mismatch_probe={} window_start_tcycles={} local_tcycles={} oracle_tcycles={}",
                    first_probe_index,
                    window_start_tcycles,
                    optional_tcycle(*local_tcycles),
                    optional_tcycle(*oracle_tcycles),
                ),
            )?;
            for mismatch in mismatches.iter().take(12) {
                writeln_checked(
                    output,
                    &format!(
                        "mismatch field={} local={} oracle={}",
                        mismatch.field, mismatch.local, mismatch.oracle
                    ),
                )?;
            }
            if mismatches.len() > 12 {
                writeln_checked(output, &format!("mismatch_more={}", mismatches.len() - 12))?;
            }
        }
    }

    Ok(())
}

fn first_divergence_outcome_name(outcome: &FirstDivergenceCaseOutcome) -> &'static str {
    match outcome {
        FirstDivergenceCaseOutcome::Matched => "matched",
        FirstDivergenceCaseOutcome::Diverged { .. } => "diverged",
    }
}

fn optional_tcycle(value: Option<u64>) -> String {
    value.map_or_else(|| "<missing>".to_string(), |value| value.to_string())
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}").map_err(|error| format!("failed to write command output: {error}"))
}

fn write_all<W: Write>(output: &mut W, text: &str) -> Result<(), String> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write command output: {error}"))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::io::{self, Write};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use crate::{
        FirstDivergenceCaseOutcome, FirstDivergenceCaseReport, FirstDivergenceSuiteReport,
        ProbeFieldMismatch, oracle_layout_root,
    };

    use super::{
        FirstDivergenceCliAction, FirstDivergenceCliOptions, default_workspace_root,
        parse_first_divergence_arguments, run_first_divergence_command, write_suite_report,
    };

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-first-divergence-cli-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    #[cfg(unix)]
    fn write_fake_sameboy_probe_runner(path: &std::path::Path) {
        fs::write(
            path,
            concat!(
                "#!/bin/sh\n",
                "set -eu\n",
                "probe_json_out=''\n",
                "while [ \"$#\" -gt 0 ]; do\n",
                "  case \"$1\" in\n",
                "    --probe-json-out)\n",
                "      shift\n",
                "      probe_json_out=\"$1\"\n",
                "      ;;\n",
                "    --probe-interval-tcycles|--timeout-tcycles|--timeout-frames|--model|--rom|--startup-cartridge-rtc-seconds)\n",
                "      shift\n",
                "      ;;\n",
                "    --write-memory)\n",
                "      shift\n",
                "      shift\n",
                "      ;;\n",
                "  esac\n",
                "  shift\n",
                "done\n",
                "if [ -n \"$probe_json_out\" ]; then\n",
                "  mkdir -p \"$(dirname \"$probe_json_out\")\"\n",
                "  cat > \"$probe_json_out\" <<'JSON'\n",
                "{\"t_cycles\":0,\"pc\":256,\"sp\":65534,\"af\":432,\"bc\":19,\"de\":216,\"hl\":333,\"ime\":false,\"div\":171,\"tima\":0,\"tma\":0,\"tac\":248,\"interrupt_flags\":225,\"interrupt_enable\":0,\"lcdc\":145,\"stat\":133,\"ly\":0,\"line_dot\":0,\"scy\":0,\"scx\":0,\"lyc\":0,\"bgp\":252,\"obp0\":255,\"obp1\":255,\"wy\":0,\"wx\":0,\"vram_hash\":\"a\",\"oam_hash\":\"b\",\"wram_hash\":\"c\",\"hram_hash\":\"d\",\"framebuffer_hash\":\"e\",\"serial_hex\":\"\"}\n",
                "JSON\n",
                "fi\n",
            ),
        )
        .expect("fake runner should be writable");
        let mut permissions = fs::metadata(path)
            .expect("fake runner metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("fake runner should be executable");
    }

    #[test]
    fn parse_arguments_supports_help_and_run_options() {
        assert_eq!(
            parse_first_divergence_arguments(["--help"]).expect("help should parse"),
            FirstDivergenceCliAction::ShowHelp
        );
        assert_eq!(
            parse_first_divergence_arguments([
                "--oracle",
                "sameboy",
                "--suite",
                "ashiepaws-dmg-curated",
                "--case",
                "ashiepaws-bully",
                "--probe-root",
                "/tmp/probes",
                "--timeout-tcycles",
                "1234",
                "--probe-interval-tcycles",
                "100",
                "--compare-mode",
                "state",
                "--build-if-missing",
            ])
            .expect("run args should parse"),
            FirstDivergenceCliAction::Run(FirstDivergenceCliOptions {
                probe_root: PathBuf::from("/tmp/probes"),
                sameboy_root: None,
                runner_binary: None,
                suite_name: "ashiepaws-dmg-curated".to_string(),
                case_id: Some("ashiepaws-bully".to_string()),
                timeout_override: Some(crate::Timeout::TCycles(1234)),
                build_if_missing: true,
                allow_divergence: false,
                compare_mode: crate::FirstDivergenceCompareMode::State,
                probe_interval_tcycles: 100,
            })
        );

        assert_eq!(
            parse_first_divergence_arguments([
                "--oracle",
                "sameboy",
                "--suite",
                "phase-2-cpu-timing",
                "--sameboy-root",
                "/tmp/SameBoy",
                "--runner-binary",
                "/tmp/runner",
                "--timeout-frames",
                "3",
                "--allow-divergence",
            ])
            .expect("root and runner args should parse"),
            FirstDivergenceCliAction::Run(FirstDivergenceCliOptions {
                probe_root: oracle_layout_root(
                    &default_workspace_root(),
                    "sameboy",
                    "first-divergence"
                ),
                sameboy_root: Some(PathBuf::from("/tmp/SameBoy")),
                runner_binary: Some(PathBuf::from("/tmp/runner")),
                suite_name: "phase-2-cpu-timing".to_string(),
                case_id: None,
                timeout_override: Some(crate::Timeout::Frames(3)),
                build_if_missing: false,
                allow_divergence: true,
                compare_mode: crate::FirstDivergenceCompareMode::Framebuffer,
                probe_interval_tcycles: 70_224,
            })
        );
    }

    #[test]
    fn parse_arguments_rejects_missing_or_bad_values() {
        let missing_oracle = parse_first_divergence_arguments(["--suite", "ashiepaws-dmg-curated"])
            .expect_err("missing oracle should fail");
        assert!(missing_oracle.contains("missing required --oracle"));

        let bad_oracle = parse_first_divergence_arguments([
            "--oracle",
            "unknown",
            "--suite",
            "ashiepaws-dmg-curated",
        ])
        .expect_err("bad oracle should fail");
        assert!(bad_oracle.contains("unknown oracle"));

        let bad_interval = parse_first_divergence_arguments([
            "--oracle",
            "sameboy",
            "--suite",
            "ashiepaws-dmg-curated",
            "--probe-interval-tcycles",
            "0",
        ])
        .expect_err("zero interval should fail");
        assert!(bad_interval.contains("greater than zero"));

        let missing_suite = parse_first_divergence_arguments(["--oracle", "sameboy"])
            .expect_err("missing suite should fail");
        assert!(missing_suite.contains("missing required --suite"));

        let unknown_argument = parse_first_divergence_arguments([
            "--oracle",
            "sameboy",
            "--suite",
            "ashiepaws-dmg-curated",
            "--unexpected",
        ])
        .expect_err("unknown argument should fail");
        assert!(unknown_argument.contains("unknown argument"));

        let missing_runner = parse_first_divergence_arguments([
            "--oracle",
            "sameboy",
            "--suite",
            "ashiepaws-dmg-curated",
            "--runner-binary",
        ])
        .expect_err("missing runner binary should fail");
        assert!(missing_runner.contains("--runner-binary requires a value"));

        let bad_timeout = parse_first_divergence_arguments([
            "--oracle",
            "sameboy",
            "--suite",
            "ashiepaws-dmg-curated",
            "--timeout-frames",
            "NaN",
        ])
        .expect_err("bad timeout should fail");
        assert!(bad_timeout.contains("invalid --timeout-frames"));

        let bad_compare = parse_first_divergence_arguments([
            "--oracle",
            "sameboy",
            "--suite",
            "ashiepaws-dmg-curated",
            "--compare-mode",
            "cpu",
        ])
        .expect_err("bad compare mode should fail");
        assert!(bad_compare.contains("unknown compare mode"));

        let default_root = parse_first_divergence_arguments([
            "--oracle",
            "sameboy",
            "--suite",
            "ashiepaws-dmg-curated",
        ])
        .expect("default root should parse");
        let FirstDivergenceCliAction::Run(default_root) = default_root else {
            panic!("expected run action");
        };
        assert_eq!(
            default_root.probe_root,
            oracle_layout_root(&default_workspace_root(), "sameboy", "first-divergence")
        );
        assert!(!default_root.allow_divergence);
    }

    #[test]
    fn suite_report_formats_match_divergence_and_writer_errors() {
        let mismatches = (0..13)
            .map(|index| ProbeFieldMismatch {
                field: format!("field{index}"),
                local: format!("local{index}"),
                oracle: format!("oracle{index}"),
            })
            .collect::<Vec<_>>();
        let report = FirstDivergenceSuiteReport {
            suite_name: "suite".to_string(),
            compare_mode: crate::FirstDivergenceCompareMode::State,
            probe_interval_tcycles: 16,
            probe_root: PathBuf::from("/tmp/probes"),
            cases: vec![
                FirstDivergenceCaseReport {
                    case_id: "matched".to_string(),
                    local_probe_path: PathBuf::from("/tmp/local-matched.jsonl"),
                    oracle_probe_path: PathBuf::from("/tmp/oracle-matched.jsonl"),
                    local_probe_count: 2,
                    oracle_probe_count: 2,
                    outcome: FirstDivergenceCaseOutcome::Matched,
                },
                FirstDivergenceCaseReport {
                    case_id: "diverged".to_string(),
                    local_probe_path: PathBuf::from("/tmp/local-diverged.jsonl"),
                    oracle_probe_path: PathBuf::from("/tmp/oracle-diverged.jsonl"),
                    local_probe_count: 3,
                    oracle_probe_count: 4,
                    outcome: FirstDivergenceCaseOutcome::Diverged {
                        first_probe_index: 2,
                        window_start_tcycles: 8,
                        local_tcycles: Some(16),
                        oracle_tcycles: None,
                        mismatches,
                    },
                },
            ],
        };

        let mut output = Vec::new();
        write_suite_report(&mut output, &report).expect("suite report should render");
        let output = String::from_utf8(output).expect("report should be utf-8");
        assert!(output.contains("suite=suite compare_mode=state"));
        assert!(output.contains("case=matched outcome=matched"));
        assert!(output.contains("case=diverged outcome=diverged"));
        assert!(output.contains("oracle_tcycles=<missing>"));
        assert!(output.contains("mismatch field=field11 local=local11 oracle=oracle11"));
        assert!(output.contains("mismatch_more=1"));

        struct FailingWriter;

        impl Write for FailingWriter {
            fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let error =
            write_suite_report(&mut FailingWriter, &report).expect_err("write error should fail");
        assert!(error.contains("failed to write command output"));
    }

    #[cfg(unix)]
    #[test]
    fn command_runs_fake_sameboy_probe_case_and_allows_divergence() {
        let temp_dir = unique_temp_dir("command");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        let fake_runner = temp_dir.join("fake-sameboy-case-bundle.sh");
        write_fake_sameboy_probe_runner(&fake_runner);
        let probe_root = temp_dir.join("probes");

        let mut output = Vec::new();
        run_first_divergence_command(
            [
                "--oracle",
                "sameboy",
                "--suite",
                "phase-2-cpu-timing",
                "--case",
                "phase2-fetch-immediate-order",
                "--probe-root",
                probe_root.to_str().expect("probe path should be utf-8"),
                "--runner-binary",
                fake_runner.to_str().expect("runner path should be utf-8"),
                "--timeout-tcycles",
                "1",
                "--probe-interval-tcycles",
                "1",
                "--compare-mode",
                "state",
                "--allow-divergence",
            ],
            &mut output,
        )
        .expect("allowed first-divergence run should succeed");

        let output = String::from_utf8(output).expect("output should be utf-8");
        assert!(output.contains("suite=phase-2-cpu-timing compare_mode=state"));
        assert!(output.contains("case=phase2-fetch-immediate-order outcome=diverged"));
        assert!(output.contains("local_probe_path="));
        assert!(probe_root.join("phase2-fetch-immediate-order").is_dir());
    }
}
