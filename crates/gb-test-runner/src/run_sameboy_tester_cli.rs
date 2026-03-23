use std::io::Write;
use std::path::PathBuf;

use crate::{
    RomRunner, RomSuite, SameBoyTesterImageFormat, SameBoyTesterRunner, Timeout,
    built_in_rom_suite_by_name, default_workspace_root, sameboy_tester_oracle_root,
};
#[derive(Debug, Clone, PartialEq, Eq)]
enum SameBoyTesterCliAction {
    ShowHelp,
    Run(SameBoyTesterCliOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SameBoyTesterCliOptions {
    oracle_root: PathBuf,
    sameboy_root: Option<PathBuf>,
    tester_binary: Option<PathBuf>,
    image_format: SameBoyTesterImageFormat,
    suite_name: String,
    case_id: Option<String>,
    timeout_override: Option<Timeout>,
    build_if_missing: bool,
}

pub fn sameboy_tester_cli_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo run -p gb-test-runner --bin run_sameboy_tester -- --suite <suite-name> [--case <case-id>] [--oracle-root <dir>] [--sameboy-root <dir> | --tester-binary <path>] [--image-format <bmp|tga>] [--timeout-frames <n> | --timeout-tcycles <n>] [--build-if-missing]\n",
        "\n",
        "The runner prepares ROM copies under the oracle root using the ROM-relative paths from the\n",
        "suite metadata, runs SameBoy Tester on those staged ROMs, and leaves BMP/TGA plus .log\n",
        "artifacts in the same tree for later use with run_differential --oracle-layout sameboy-tester.\n",
        "If --oracle-root is omitted, the default repo-local root is .oracles/sameboy/sameboy-tester/.\n",
        "This command intentionally does not override SameBoy's own boot-ROM path.\n",
        "\n",
        "Environment fallbacks:\n",
        "  GB_CYCLE_SAMEBOY_ROOT for --sameboy-root\n",
        "  GB_CYCLE_SAMEBOY_TESTER_BIN for --tester-binary\n",
    )
}

pub fn run_sameboy_tester_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_sameboy_tester_arguments(arguments)? {
        SameBoyTesterCliAction::ShowHelp => write_all(output, sameboy_tester_cli_help_text()),
        SameBoyTesterCliAction::Run(options) => run_selected_suite(options, output),
    }
}

fn parse_sameboy_tester_arguments<I, S>(arguments: I) -> Result<SameBoyTesterCliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut oracle_root = None;
    let mut sameboy_root = None;
    let mut tester_binary = None;
    let mut image_format = SameBoyTesterImageFormat::Bmp;
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
            "--tester-binary" => {
                let Some(value) = arguments.next() else {
                    return Err("--tester-binary requires a value".to_string());
                };
                tester_binary = Some(PathBuf::from(value.as_ref()));
            }
            "--image-format" => {
                let Some(value) = arguments.next() else {
                    return Err("--image-format requires a value".to_string());
                };
                image_format = parse_image_format(value.as_ref())?;
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
            "--help" | "-h" => return Ok(SameBoyTesterCliAction::ShowHelp),
            other => return Err(format!("unknown argument {other:?}; run with --help")),
        }
    }

    let Some(suite_name) = suite_name else {
        return Err(
            "missing required --suite <suite-name>; run run_rom_suite -- --list".to_string(),
        );
    };

    Ok(SameBoyTesterCliAction::Run(SameBoyTesterCliOptions {
        oracle_root: oracle_root
            .unwrap_or_else(|| sameboy_tester_oracle_root(&default_workspace_root())),
        sameboy_root,
        tester_binary,
        image_format,
        suite_name,
        case_id,
        timeout_override,
        build_if_missing,
    }))
}

fn parse_image_format(value: &str) -> Result<SameBoyTesterImageFormat, String> {
    match value {
        "bmp" => Ok(SameBoyTesterImageFormat::Bmp),
        "tga" => Ok(SameBoyTesterImageFormat::Tga),
        _ => Err(format!(
            "unknown image format {value:?}; expected bmp or tga"
        )),
    }
}

fn run_selected_suite<W: Write>(
    options: SameBoyTesterCliOptions,
    output: &mut W,
) -> Result<(), String> {
    let mut suite = select_suite_for_options(&options)?;
    if let Some(timeout_override) = options.timeout_override {
        for case in &mut suite.cases {
            case.timeout = timeout_override;
        }
    }

    let mut runner = SameBoyTesterRunner::new(&options.oracle_root)
        .with_rom_runner(RomRunner::new())
        .with_image_format(options.image_format)
        .with_build_if_missing(options.build_if_missing);
    if let Some(sameboy_root) = options.sameboy_root {
        runner = runner.with_sameboy_root(sameboy_root);
    }
    if let Some(tester_binary) = options.tester_binary {
        runner = runner.with_tester_binary(tester_binary);
    }

    let report = runner.run_suite(&suite).map_err(|error| {
        format!(
            "failed to execute SameBoy Tester suite {}: {error:?}",
            suite.name
        )
    })?;
    write_suite_report(output, &report)
}

fn select_suite_for_options(options: &SameBoyTesterCliOptions) -> Result<RomSuite, String> {
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
    report: &crate::SameBoyTesterSuiteReport,
) -> Result<(), String> {
    writeln_checked(
        output,
        &format!(
            "suite={} subsystem={:?} tester_binary={} oracle_root={} image_format={}",
            report.suite_name,
            report.subsystem,
            report.tester_binary.display(),
            report.oracle_root.display(),
            report.image_format.name(),
        ),
    )?;

    for case in &report.cases {
        writeln_checked(
            output,
            &format!(
                "case={} staged_rom={} image_artifact={} log_artifact={}",
                case.case_id,
                case.staged_rom_path.display(),
                case.image_artifact_path.display(),
                case.log_artifact_path
                    .as_ref()
                    .map_or_else(|| "-".to_string(), |path| path.display().to_string()),
            ),
        )?;
        if let Some(note) = &case.startup_mode_note {
            writeln_checked(output, &format!("startup_mode_note={note}"))?;
        }
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
    use std::env;
    use std::fs;
    use std::io;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        SameBoyTesterImageFormat, SameBoyTesterSuiteReport, TEST_ROM_ROOT_ENV_VAR, TestSubsystem,
        sameboy_tester::SameBoyTesterCaseReport, sameboy_tester_oracle_root,
    };

    use super::{
        SameBoyTesterCliAction, SameBoyTesterCliOptions, default_workspace_root,
        parse_sameboy_tester_arguments, run_sameboy_tester_command, sameboy_tester_cli_help_text,
        select_suite_for_options, write_suite_report,
    };

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-run-sameboy-tester-cli-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore the touched variables before dropping the guard.
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore the touched variables before dropping the guard.
        unsafe {
            env::remove_var(key);
        }
    }

    fn write_executable(path: &std::path::Path, body: &str) {
        fs::write(path, body).expect("script should be writable");
        let mut permissions = fs::metadata(path)
            .expect("script metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("script should be executable");
    }

    #[test]
    fn parse_arguments_supports_image_format_and_build_flag() {
        assert_eq!(
            parse_sameboy_tester_arguments(["--help"]).expect("help should parse"),
            SameBoyTesterCliAction::ShowHelp
        );
        assert_eq!(
            parse_sameboy_tester_arguments([
                "--sameboy-root",
                "/tmp/SameBoy",
                "--image-format",
                "tga",
                "--suite",
                "acid-dmg-curated",
                "--case",
                "dmg-acid2",
                "--build-if-missing",
            ])
            .expect("run args should parse"),
            SameBoyTesterCliAction::Run(SameBoyTesterCliOptions {
                oracle_root: sameboy_tester_oracle_root(&default_workspace_root()),
                sameboy_root: Some(PathBuf::from("/tmp/SameBoy")),
                tester_binary: None,
                image_format: crate::SameBoyTesterImageFormat::Tga,
                suite_name: "acid-dmg-curated".to_string(),
                case_id: Some("dmg-acid2".to_string()),
                timeout_override: None,
                build_if_missing: true,
            })
        );
    }

    #[test]
    fn parse_arguments_rejects_bad_values_and_defaults_oracle_root() {
        let parsed = parse_sameboy_tester_arguments(["--suite", "acid-dmg-curated"])
            .expect("suite-only invocation should use default oracle root");
        let SameBoyTesterCliAction::Run(parsed) = parsed else {
            panic!("expected run action");
        };
        assert_eq!(
            parsed.oracle_root,
            sameboy_tester_oracle_root(&default_workspace_root())
        );
        assert_eq!(
            sameboy_tester_cli_help_text(),
            parse_sameboy_tester_arguments(["-h"])
                .map(|action| match action {
                    SameBoyTesterCliAction::ShowHelp => sameboy_tester_cli_help_text(),
                    SameBoyTesterCliAction::Run(_) => panic!("expected help action"),
                })
                .expect("short help should parse")
        );

        let bad_format = parse_sameboy_tester_arguments([
            "--image-format",
            "png",
            "--suite",
            "acid-dmg-curated",
        ])
        .expect_err("bad image format should fail");
        assert!(bad_format.contains("unknown image format"));
    }

    #[test]
    fn select_suite_rejects_unknown_suite_or_case() {
        let unknown_suite = select_suite_for_options(&SameBoyTesterCliOptions {
            oracle_root: PathBuf::from("/tmp/oracle"),
            sameboy_root: None,
            tester_binary: None,
            image_format: crate::SameBoyTesterImageFormat::Bmp,
            suite_name: "unknown".to_string(),
            case_id: None,
            timeout_override: None,
            build_if_missing: false,
        })
        .expect_err("unknown suite should fail");
        assert!(unknown_suite.contains("unknown suite"));

        let unknown_case = select_suite_for_options(&SameBoyTesterCliOptions {
            oracle_root: PathBuf::from("/tmp/oracle"),
            sameboy_root: None,
            tester_binary: None,
            image_format: crate::SameBoyTesterImageFormat::Bmp,
            suite_name: "acid-dmg-curated".to_string(),
            case_id: Some("missing".to_string()),
            timeout_override: None,
            build_if_missing: false,
        })
        .expect_err("unknown case should fail");
        assert!(unknown_case.contains("does not contain case"));
    }

    #[test]
    fn run_command_executes_suite_and_renders_report_lines() {
        let _guard = env_lock().lock().expect("env lock should be lockable");
        let temp_dir = unique_temp_dir("run-command");
        let oracle_root = temp_dir.join("oracle");
        let rom_root = temp_dir.join("test-rom-store");
        let rom_path = rom_root.join("acid/dmg-acid2.gb");
        fs::create_dir_all(rom_path.parent().expect("rom path should have a parent"))
            .expect("rom dir should be creatable");
        fs::write(&rom_path, b"rom").expect("rom should be writable");

        let tester_binary = temp_dir.join("sameboy-tester");
        write_executable(
            &tester_binary,
            "#!/bin/sh\nset -eu\nrom=''\nfor arg in \"$@\"; do rom=\"$arg\"; done\nprintf 'bmp' > \"${rom%.gb}.bmp\"\nprintf 'log' > \"${rom%.gb}.log\"\n",
        );
        set_env_var(TEST_ROM_ROOT_ENV_VAR, &rom_root);

        let mut output = Vec::new();
        run_sameboy_tester_command(
            [
                "--suite",
                "acid-dmg-curated",
                "--case",
                "dmg-acid2",
                "--oracle-root",
                oracle_root.to_str().expect("oracle root should be utf-8"),
                "--tester-binary",
                tester_binary
                    .to_str()
                    .expect("tester binary path should be utf-8"),
            ],
            &mut output,
        )
        .expect("run command should succeed");
        remove_env_var(TEST_ROM_ROOT_ENV_VAR);

        let rendered = String::from_utf8(output).expect("output should be utf-8");
        assert!(rendered.contains("suite=acid-dmg-curated"));
        assert!(rendered.contains("case=dmg-acid2"));
        assert!(rendered.contains("image_format=bmp"));
        assert!(
            rendered
                .contains("startup_mode_note=SameBoy Tester always executes through a boot ROM")
        );
    }

    #[test]
    fn run_command_reports_selection_and_output_errors() {
        let unknown_suite = run_sameboy_tester_command(["--suite", "missing"], &mut io::sink())
            .expect_err("unknown suite should fail");
        assert!(unknown_suite.contains("unknown suite"));

        struct BrokenWriter;

        impl io::Write for BrokenWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("broken writer"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let error = run_sameboy_tester_command(["--help"], &mut BrokenWriter)
            .expect_err("broken writer should surface command output errors");
        assert!(error.contains("failed to write command output"));
    }

    #[test]
    fn write_suite_report_renders_case_lines_and_writer_failures() {
        let report = SameBoyTesterSuiteReport {
            suite_name: "suite".to_string(),
            subsystem: TestSubsystem::Ppu,
            tester_binary: PathBuf::from("/tmp/sameboy_tester"),
            oracle_root: PathBuf::from("/tmp/oracles"),
            image_format: SameBoyTesterImageFormat::Bmp,
            cases: vec![SameBoyTesterCaseReport {
                case_id: "case".to_string(),
                staged_rom_path: PathBuf::from("/tmp/oracles/test.gb"),
                image_artifact_path: PathBuf::from("/tmp/oracles/test.bmp"),
                log_artifact_path: Some(PathBuf::from("/tmp/oracles/test.log")),
                startup_mode_note: Some("note".to_string()),
            }],
        };

        let mut output = Vec::new();
        write_suite_report(&mut output, &report).expect("report should render");
        let output = String::from_utf8(output).expect("report should be utf-8");
        assert!(output.contains("suite=suite subsystem=Ppu"));
        assert!(output.contains("case=case"));
        assert!(output.contains("startup_mode_note=note"));

        struct BrokenWriter;

        impl io::Write for BrokenWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("broken writer"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let error = write_suite_report(&mut BrokenWriter, &report)
            .expect_err("writer failure should surface");
        assert!(error.contains("failed to write command output"));
        assert!(sameboy_tester_cli_help_text().contains("sameboy-tester"));
    }
}
