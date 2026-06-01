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
