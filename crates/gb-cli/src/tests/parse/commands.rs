use super::super::*;

#[test]
fn run_cli_command_routes_help_variants_and_unknown_subcommands() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(std::iter::empty::<&str>(), &mut stdout, &mut stderr)
        .expect("empty CLI should print help");
    let output = String::from_utf8(stdout.clone()).expect("stdout should be UTF-8");
    assert!(output.contains("Commands:\n"));
    assert!(stderr.is_empty());

    stdout.clear();
    stderr.clear();
    run_cli_command(["run", "--help"], &mut stdout, &mut stderr).expect("run help should succeed");
    assert_eq!(
        String::from_utf8(stdout.clone()).expect("stdout should be UTF-8"),
        RUN_HELP_TEXT
    );
    assert!(stderr.is_empty());

    stdout.clear();
    stderr.clear();
    run_cli_command(["inspect-rom", "--help"], &mut stdout, &mut stderr)
        .expect("inspect help should succeed");
    assert_eq!(
        String::from_utf8(stdout.clone()).expect("stdout should be UTF-8"),
        INSPECT_HELP_TEXT
    );
    assert!(stderr.is_empty());

    stdout.clear();
    stderr.clear();
    run_cli_command(["saves", "--help"], &mut stdout, &mut stderr)
        .expect("saves help should succeed");
    assert_eq!(
        String::from_utf8(stdout).expect("stdout should be UTF-8"),
        SAVES_HELP_TEXT
    );
    assert!(stderr.is_empty());

    assert_eq!(
        run_cli_command(["wat"], &mut Vec::new(), &mut Vec::new())
            .expect_err("unknown subcommands should fail"),
        "unknown subcommand \"wat\"; run `gb-cli --help` for usage"
    );
}

#[test]
fn parse_inspect_rom_arguments_cover_valid_help_and_error_paths() {
    match parse_inspect_rom_arguments(["demo.gb", "--mode", "experimental"])
        .expect("inspect arguments should parse")
    {
        CliAction::InspectRom(options) => {
            assert_eq!(options.rom_path, PathBuf::from("demo.gb"));
            assert_eq!(options.execution_mode, ExecutionMode::Experimental);
        }
        other => panic!("expected inspect action, got {other:?}"),
    }

    assert_eq!(
        parse_inspect_rom_arguments(["--help"]).expect("help should parse"),
        CliAction::ShowInspectHelp
    );
    assert_eq!(
        parse_inspect_rom_arguments(["demo.gb", "--mode"])
            .expect_err("mode should require a value"),
        "--mode requires a value"
    );
    assert_eq!(
        parse_inspect_rom_arguments(["demo.gb", "--weird"])
            .expect_err("unknown inspect options should fail"),
        "unknown inspect-rom option \"--weird\"; run `gb-cli inspect-rom --help`"
    );
    assert_eq!(
        parse_inspect_rom_arguments(["demo.gb", "other.gb"])
            .expect_err("extra positional arguments should fail"),
        "unexpected extra positional argument \"other.gb\"; run `gb-cli inspect-rom --help`"
    );
    assert_eq!(
        parse_inspect_rom_arguments(std::iter::empty::<&str>())
            .expect_err("inspect requires a ROM path"),
        "missing required ROM path; run `gb-cli inspect-rom --help` for usage"
    );
}

#[test]
fn parse_saves_arguments_cover_valid_help_and_error_paths() {
    match parse_saves_arguments([
        "export",
        "demo.gb",
        "demo.sav",
        "--save-dir",
        "saves",
        "--save-key",
        "slot1",
    ])
    .expect("saves export arguments should parse")
    {
        CliAction::Saves(options) => {
            assert_eq!(options.direction, SavesDirection::Export);
            assert_eq!(options.rom_path, PathBuf::from("demo.gb"));
            assert_eq!(options.external_save_path, PathBuf::from("demo.sav"));
            assert_eq!(options.save_dir, PathBuf::from("saves"));
            assert_eq!(options.save_key.as_deref(), Some("slot1"));
        }
        other => panic!("expected saves action, got {other:?}"),
    }

    match parse_saves_arguments(["import", "demo.gb", "demo.sav", "--save-dir", "saves"])
        .expect("saves import arguments should parse")
    {
        CliAction::Saves(options) => {
            assert_eq!(options.direction, SavesDirection::Import);
            assert_eq!(options.save_key, None);
        }
        other => panic!("expected saves action, got {other:?}"),
    }

    assert_eq!(
        parse_saves_arguments(["--help"]).expect("help should parse"),
        CliAction::ShowSavesHelp
    );
    assert_eq!(
        parse_saves_arguments(std::iter::empty::<&str>()).expect_err("missing action should fail"),
        "missing saves action; run `gb-cli saves --help` for usage"
    );
    assert_eq!(
        parse_saves_arguments(["copy", "demo.gb", "demo.sav", "--save-dir", "saves"])
            .expect_err("unknown action should fail"),
        "unknown saves action \"copy\"; expected export or import"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "demo.sav"])
            .expect_err("save dir should be required"),
        "--save-dir is required"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "demo.sav", "--save-dir"])
            .expect_err("save dir value should be required"),
        "--save-dir requires a value"
    );
    assert_eq!(
        parse_saves_arguments([
            "export",
            "demo.gb",
            "demo.sav",
            "--save-dir",
            "saves",
            "--save-key"
        ])
        .expect_err("save key value should be required"),
        "--save-key requires a value"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "--save-dir", "saves"])
            .expect_err("both positional paths should be required"),
        "missing required ROM path or .sav path; run `gb-cli saves --help` for usage"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "demo.sav", "--weird"])
            .expect_err("unknown option should fail"),
        "unknown saves option \"--weird\"; run `gb-cli saves --help`"
    );
    assert_eq!(
        parse_saves_arguments(["export", "demo.gb", "demo.sav", "extra"])
            .expect_err("extra positional should fail"),
        "unexpected extra positional argument \"extra\"; run `gb-cli saves --help`"
    );
}
