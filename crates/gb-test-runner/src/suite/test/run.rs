use std::fs;

use super::super::cli::run_suite_command_with_workspace_for_test;
use super::common::{
    basic_manifest, build_serial_text_rom, unique_temp_dir, write_manifest, write_reports,
};

#[test]
fn command_runs_serial_suite_and_writes_status() {
    let workspace = unique_temp_dir("serial-pass");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );
    write_manifest(
        &workspace,
        "sample-report/acid.toml",
        &basic_manifest("acid", "acid", "acid-cgb-acid2", "cgb-acid2.gbc")
            .replace("console = \"dmg\"", "console = \"cgb\"")
            .replace("serial-contains", "framebuffer-fixture"),
    );
    let rom_path = workspace.join("test/sample-report/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("Passed")).expect("rom should be writable");

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "blargg-cpu-instrs",
            "--case",
            "blargg-cpu-instrs-01-special",
        ],
        &workspace,
        &mut output,
    )
    .expect("suite should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite blargg-cpu-instrs: 1/1 passed"));
    let status =
        fs::read_to_string(workspace.join("test/sample-report/.status/blargg-cpu-instrs.toml"))
            .expect("status should be written");
    assert!(status.contains("suite_name = \"blargg-cpu-instrs\""));
    assert!(status.contains("status = \"PASS\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_runs_with_explicit_threads_and_preserves_status_order() {
    let workspace = unique_temp_dir("serial-threads");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/threaded.suite.toml",
        r#"
family = "blargg"
suite_name = "threaded"
console = "dmg"
timeout_frames = 2
oracle = { type = "serial-contains", expected = "Passed" }

[[case]]
id = "threaded-first"
rom = "threaded/first.gb"

[[case]]
id = "threaded-second"
rom = "threaded/second.gb"
"#,
    );
    for rom in ["first.gb", "second.gb"] {
        let rom_path = workspace
            .join("test/sample-report/blargg/threaded")
            .join(rom);
        fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
            .expect("rom parent should be creatable");
        fs::write(&rom_path, build_serial_text_rom("Passed")).expect("rom should be writable");
    }

    for threads in ["2", "1"] {
        let mut output = Vec::new();
        run_suite_command_with_workspace_for_test(
            ["sample-report", "--suite", "threaded", "--threads", threads],
            &workspace,
            &mut output,
        )
        .expect("threaded suite should pass");
        let output = String::from_utf8(output).expect("output should be utf-8");
        assert!(output.contains("suite threaded: 2/2 passed"));
    }

    let status = fs::read_to_string(workspace.join("test/sample-report/.status/threaded.toml"))
        .expect("status should be written");
    let first = status
        .find("rom = \"threaded/first.gb\"")
        .expect("first row should exist");
    let second = status
        .find("rom = \"threaded/second.gb\"")
        .expect("second row should exist");
    assert!(first < second);

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_reports_failed_cases_and_rejects_unknown_case() {
    let workspace = unique_temp_dir("serial-fail");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/blargg-cpu-instrs.suite.toml",
        &basic_manifest(
            "blargg-cpu-instrs",
            "blargg",
            "blargg-cpu-instrs-01-special",
            "cpu_instrs/01-special.gb",
        ),
    );
    let rom_path = workspace.join("test/sample-report/blargg/cpu_instrs/01-special.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("Failed")).expect("rom should be writable");

    let mut output = Vec::new();
    let error = run_suite_command_with_workspace_for_test(
        ["sample-report", "--suite", "blargg-cpu-instrs"],
        &workspace,
        &mut output,
    )
    .expect_err("suite should fail");
    assert!(error.contains("one or more suite cases failed"));
    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("case blargg-cpu-instrs-01-special: FAIL"));
    let status =
        fs::read_to_string(workspace.join("test/sample-report/.status/blargg-cpu-instrs.toml"))
            .expect("status should be written");
    assert!(status.contains("status = \"FAIL\""));

    let mut output = Vec::new();
    assert!(
        run_suite_command_with_workspace_for_test(
            [
                "sample-report",
                "--suite",
                "blargg-cpu-instrs",
                "--case",
                "missing-case",
            ],
            &workspace,
            &mut output,
        )
        .expect_err("unknown case should fail")
        .contains("unknown case")
    );

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}

#[test]
fn command_treats_info_framebuffer_as_pass_for_ci() {
    let workspace = unique_temp_dir("framebuffer-info-pass");
    write_reports(
        &workspace,
        "sample-report",
        "sample-report/sources.report.toml",
    );
    write_manifest(
        &workspace,
        "sample-report/acid.suite.toml",
        &basic_manifest("acid", "acid", "acid-which-dmg", "which.gb")
            .replace(
                "oracle = { type = \"serial-contains\", expected = \"Passed\" }",
                "oracle = { type = \"framebuffer\", mode = \"info\" }",
            )
            .replace("timeout_frames = 2", "timeout_frames = 1"),
    );
    let rom_path = workspace.join("test/sample-report/acid/which.gb");
    fs::create_dir_all(rom_path.parent().expect("rom should have parent"))
        .expect("rom parent should be creatable");
    fs::write(&rom_path, build_serial_text_rom("")).expect("rom should be writable");

    let mut output = Vec::new();
    run_suite_command_with_workspace_for_test(
        [
            "sample-report",
            "--suite",
            "acid",
            "--case",
            "acid-which-dmg",
        ],
        &workspace,
        &mut output,
    )
    .expect("info framebuffer should pass");

    let output = String::from_utf8(output).expect("output should be utf-8");
    assert!(output.contains("suite acid: 1/1 passed"));
    let status = fs::read_to_string(workspace.join("test/sample-report/.status/acid.toml"))
        .expect("status should be written");
    assert!(status.contains("status = \"PASS\""));

    fs::remove_dir_all(workspace).expect("workspace should be removable");
}
