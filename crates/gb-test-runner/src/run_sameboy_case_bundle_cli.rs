use std::io::Write;
use std::path::PathBuf;

use crate::{
    RomRunner, RomSuite, SameBoyCaseBundleRunner, Timeout, built_in_rom_suite_by_name,
    default_workspace_root, sameboy_case_bundle_oracle_root,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum SameBoyCaseBundleCliAction {
    ShowHelp,
    Run(SameBoyCaseBundleCliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SameBoyCaseBundleCliOptions {
    oracle_root: PathBuf,
    sameboy_root: Option<PathBuf>,
    runner_binary: Option<PathBuf>,
    suite_name: String,
    case_id: Option<String>,
    timeout_override: Option<Timeout>,
    build_if_missing: bool,
}

pub fn sameboy_case_bundle_cli_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- --suite <suite-name> [--case <case-id>] [--oracle-root <dir>] [--sameboy-root <dir> | --runner-binary <path>] [--timeout-frames <n> | --timeout-tcycles <n>] [--build-if-missing]\n",
        "\n",
        "The runner materializes SameBoy case-bundle artifacts, such as serial_hex.txt,\n",
        "under one subdirectory per case id. If --oracle-root is omitted, the default\n",
        "repo-local root is .oracles/sameboy/case-bundle/.\n",
        "\n",
        "Environment fallbacks:\n",
        "  GB_CYCLE_SAMEBOY_ROOT for --sameboy-root\n",
        "  GB_CYCLE_SAMEBOY_CASE_BUNDLE_BIN for --runner-binary\n",
    )
}

pub fn run_sameboy_case_bundle_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_sameboy_case_bundle_arguments(arguments)? {
        SameBoyCaseBundleCliAction::ShowHelp => {
            write_all(output, sameboy_case_bundle_cli_help_text())
        }
        SameBoyCaseBundleCliAction::Run(options) => run_selected_suite(options, output),
    }
}

fn parse_sameboy_case_bundle_arguments<I, S>(
    arguments: I,
) -> Result<SameBoyCaseBundleCliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut oracle_root = None;
    let mut sameboy_root = None;
    let mut runner_binary = None;
    let mut suite_name = None;
    let mut case_id = None;
    let mut timeout_override = None;
    let mut build_if_missing = false;

    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--oracle-root" => {
                let Some(value) = arguments.next() else {
                    return Err("--oracle-root requires a value".to_string());
                };
                oracle_root = Some(PathBuf::from(value.as_ref()));
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
            "--build-if-missing" => {
                build_if_missing = true;
            }
            "--help" | "-h" => return Ok(SameBoyCaseBundleCliAction::ShowHelp),
            other => return Err(format!("unknown argument {other:?}; run with --help")),
        }
    }

    let Some(suite_name) = suite_name else {
        return Err(
            "missing required --suite <suite-name>; run run_rom_suite -- --list".to_string(),
        );
    };

    Ok(SameBoyCaseBundleCliAction::Run(
        SameBoyCaseBundleCliOptions {
            oracle_root: oracle_root
                .unwrap_or_else(|| sameboy_case_bundle_oracle_root(&default_workspace_root())),
            sameboy_root,
            runner_binary,
            suite_name,
            case_id,
            timeout_override,
            build_if_missing,
        },
    ))
}

fn run_selected_suite<W: Write>(
    options: SameBoyCaseBundleCliOptions,
    output: &mut W,
) -> Result<(), String> {
    let mut suite = select_suite_for_options(&options)?;
    if let Some(timeout_override) = options.timeout_override {
        for case in &mut suite.cases {
            case.timeout = timeout_override;
        }
    }

    let mut runner = SameBoyCaseBundleRunner::new(&options.oracle_root)
        .with_rom_runner(RomRunner::new())
        .with_build_if_missing(options.build_if_missing);
    if let Some(sameboy_root) = options.sameboy_root {
        runner = runner.with_sameboy_root(sameboy_root);
    }
    if let Some(runner_binary) = options.runner_binary {
        runner = runner.with_runner_binary(runner_binary);
    }

    let report = runner.run_suite(&suite).map_err(|error| {
        format!(
            "failed to execute SameBoy case-bundle suite {}: {error:?}",
            suite.name
        )
    })?;
    write_suite_report(output, &report)
}

fn select_suite_for_options(options: &SameBoyCaseBundleCliOptions) -> Result<RomSuite, String> {
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
    report: &crate::SameBoyCaseBundleSuiteReport,
) -> Result<(), String> {
    writeln_checked(
        output,
        &format!(
            "suite={} runner_binary={} oracle_root={}",
            report.suite_name,
            report.runner_binary.display(),
            report.oracle_root.display(),
        ),
    )?;

    for case in &report.cases {
        writeln_checked(
            output,
            &format!(
                "case={} rom={} serial_hex_artifact={}",
                case.case_id,
                case.rom_path.display(),
                case.serial_hex_artifact_path.display(),
            ),
        )?;
    }

    Ok(())
}

fn writeln_checked<W: Write>(output: &mut W, line: &str) -> Result<(), String> {
    writeln!(output, "{line}").map_err(|error| format!("failed to write output: {error}"))
}

fn write_all<W: Write>(output: &mut W, text: &str) -> Result<(), String> {
    output
        .write_all(text.as_bytes())
        .map_err(|error| format!("failed to write output: {error}"))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        SameBoyCaseBundleSuiteReport, sameboy_case_bundle::SameBoyCaseBundleCaseReport,
        sameboy_case_bundle_cli_help_text, sameboy_case_bundle_oracle_root,
    };

    use super::{
        SameBoyCaseBundleCliAction, SameBoyCaseBundleCliOptions,
        parse_sameboy_case_bundle_arguments, run_sameboy_case_bundle_command,
        select_suite_for_options,
    };

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-sameboy-case-bundle-cli-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn write_fake_runner(path: &std::path::Path, args_output: &std::path::Path) {
        fs::write(
            path,
            format!(
                concat!(
                    "#!/bin/sh\n",
                    "set -eu\n",
                    "args_file=\"{}\"\n",
                    "printf '%s\\n' '---' >> \"$args_file\"\n",
                    "serial_hex_out=''\n",
                    "while [ \"$#\" -gt 0 ]; do\n",
                    "  printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "  if [ \"$1\" = '--serial-hex-out' ]; then\n",
                    "    shift\n",
                    "    printf '%s\\n' \"$1\" >> \"$args_file\"\n",
                    "    serial_hex_out=\"$1\"\n",
                    "  fi\n",
                    "  shift\n",
                    "done\n",
                    "printf 'CLIHEX' > \"$serial_hex_out\"\n",
                ),
                args_output.display(),
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
    fn parser_applies_case_bundle_defaults() {
        let parsed = parse_sameboy_case_bundle_arguments(["--suite", "phase-6-cartridge-oracle"])
            .expect("defaults should parse");
        let SameBoyCaseBundleCliAction::Run(parsed) = parsed else {
            panic!("expected run action");
        };
        assert_eq!(
            parsed.oracle_root,
            sameboy_case_bundle_oracle_root(&crate::default_workspace_root())
        );
    }

    #[test]
    fn parser_accepts_runner_binary_and_timeout_override() {
        let parsed = parse_sameboy_case_bundle_arguments([
            "--suite",
            "phase-6-cartridge-oracle",
            "--runner-binary",
            "/tmp/sameboy-case-bundle",
            "--timeout-tcycles",
            "1234",
            "--build-if-missing",
        ])
        .expect("arguments should parse");
        assert_eq!(
            parsed,
            SameBoyCaseBundleCliAction::Run(SameBoyCaseBundleCliOptions {
                oracle_root: sameboy_case_bundle_oracle_root(&crate::default_workspace_root()),
                sameboy_root: None,
                runner_binary: Some(PathBuf::from("/tmp/sameboy-case-bundle")),
                suite_name: "phase-6-cartridge-oracle".to_string(),
                case_id: None,
                timeout_override: Some(crate::Timeout::TCycles(1234)),
                build_if_missing: true,
            })
        );
    }

    #[test]
    fn parser_rejects_missing_values_and_unknown_arguments() {
        let missing_case =
            parse_sameboy_case_bundle_arguments(["--suite", "phase-6-cartridge-oracle", "--case"])
                .expect_err("missing --case value should fail");
        assert!(missing_case.contains("--case requires a value"));

        let invalid_timeout = parse_sameboy_case_bundle_arguments([
            "--suite",
            "phase-6-cartridge-oracle",
            "--timeout-frames",
            "nope",
        ])
        .expect_err("invalid timeout should fail");
        assert!(invalid_timeout.contains("invalid --timeout-frames value"));

        let unknown_argument =
            parse_sameboy_case_bundle_arguments(["--suite", "phase-6-cartridge-oracle", "--bogus"])
                .expect_err("unknown argument should fail");
        assert!(unknown_argument.contains("unknown argument"));
    }

    #[test]
    fn select_suite_can_filter_a_family_backed_built_in_suite_to_one_case() {
        let suite = select_suite_for_options(&SameBoyCaseBundleCliOptions {
            oracle_root: PathBuf::from("/tmp/oracle"),
            sameboy_root: None,
            runner_binary: None,
            suite_name: "acid-dmg-curated".to_string(),
            case_id: Some("dmg-acid2".to_string()),
            timeout_override: None,
            build_if_missing: false,
        })
        .expect("known curated case should be selectable");

        assert_eq!(suite.name, "acid-dmg-curated");
        assert_eq!(suite.family.as_deref(), Some("acid"));
        assert_eq!(suite.cases.len(), 1);
        assert_eq!(suite.cases[0].id, "dmg-acid2");
    }

    #[test]
    fn help_and_report_paths_render_expected_labels() {
        assert!(sameboy_case_bundle_cli_help_text().contains("run_sameboy_case_bundle"));

        let report = SameBoyCaseBundleSuiteReport {
            suite_name: "phase-6-cartridge-oracle".to_string(),
            runner_binary: PathBuf::from("/tmp/runner"),
            oracle_root: PathBuf::from("/tmp/oracle"),
            cases: vec![SameBoyCaseBundleCaseReport {
                case_id: "phase6-mbc1-standard-banking".to_string(),
                rom_path: PathBuf::from("rom.gb"),
                serial_hex_artifact_path: PathBuf::from("/tmp/oracle/serial_hex.txt"),
            }],
        };
        let mut output = Vec::new();
        super::write_suite_report(&mut output, &report).expect("report should write");
        let output = String::from_utf8(output).expect("output should be utf-8");
        assert!(output.contains("suite=phase-6-cartridge-oracle"));
        assert!(output.contains("serial_hex_artifact=/tmp/oracle/serial_hex.txt"));
    }

    struct BrokenWriter;

    impl Write for BrokenWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("broken"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn broken_writer_surfaces_as_error() {
        let error = run_sameboy_case_bundle_command(["--help"], &mut BrokenWriter)
            .expect_err("broken writer should fail");
        assert!(error.contains("failed to write output"));
    }

    #[test]
    fn run_command_executes_selected_case_and_renders_report() {
        let temp_dir = unique_temp_dir("run-command");
        let oracle_root = temp_dir.join("oracle");
        let args_output = temp_dir.join("runner-args.txt");
        let runner_binary = temp_dir.join("fake-sameboy-case-bundle.sh");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
        write_fake_runner(&runner_binary, &args_output);

        let mut output = Vec::new();
        run_sameboy_case_bundle_command(
            [
                "--suite",
                "phase-6-cartridge-oracle",
                "--case",
                "phase6-mbc1-standard-banking",
                "--oracle-root",
                oracle_root.to_str().expect("oracle root should be utf-8"),
                "--runner-binary",
                runner_binary.to_str().expect("runner path should be utf-8"),
                "--timeout-frames",
                "12",
            ],
            &mut output,
        )
        .expect("command should succeed");

        let output = String::from_utf8(output).expect("command output should be utf-8");
        assert!(output.contains("suite=phase-6-cartridge-oracle"));
        assert!(output.contains("case=phase6-mbc1-standard-banking"));
        assert!(output.contains("serial_hex_artifact="));

        let args = fs::read_to_string(args_output).expect("runner args should be readable");
        assert!(args.contains("--timeout-frames\n12\n"));
        assert!(!args.contains("--timeout-tcycles"));
    }

    #[test]
    fn run_command_reports_unknown_suite_and_case_selection_errors() {
        let unknown_suite =
            run_sameboy_case_bundle_command(["--suite", "unknown"], &mut Vec::new())
                .expect_err("unknown suite should fail");
        assert!(unknown_suite.contains("unknown suite"));

        let unknown_case = run_sameboy_case_bundle_command(
            [
                "--suite",
                "phase-6-cartridge-oracle",
                "--case",
                "missing-case",
            ],
            &mut Vec::new(),
        )
        .expect_err("unknown case should fail");
        assert!(unknown_case.contains("does not contain case"));
    }
}
